use teloxide::dispatching::dialogue::GetChatId;
use teloxide::{prelude::*, utils::command::BotCommands};
use teloxide::types::{KeyboardButton, KeyboardMarkup, Message, ParseMode, ReplyMarkup, InputFile};
use teloxide::dispatching::{dialogue::enter, dialogue::InMemStorage, UpdateHandler};
use tracing::{info, error};
use tracing_subscriber;

use dotenv::dotenv;
use std::env;

use csv::Writer;
use serde::Serialize;

use sqlx::{SqlitePool, FromRow};

// use google_sheets4::Sheets; // Пример для Google Sheets API
use std::sync::Arc;

type MyDialogue = Dialogue<FormState, InMemStorage<FormState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, FromRow, Serialize)]
struct User {
    id: i64,
    chat_id: i64,       // ID чата
    username: Option<String>,   // Имя пользователя (например, из Telegram)
    name: String,       // Имя пользователя
    course: String,     // Курс
    question: String,   // Вопрос
}

#[derive(Clone, Default)]
pub enum FormState {
    #[default]
    Start,
    ReceiveFullName,
    ReceiveCourse {
        full_name: String,
    },
    ReceiveQuestion {
        full_name: String,
        course: String,
    },
}

/// Доступные команды:
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "Показать это сообщение.")]
    Help,
    #[command(description = "Начать")]
    Start,
    #[command(description = "Отмена")]
    Cancel,
}

/// Доступные команды для админа:
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum CommandAdmin {
    #[command(description = "Список организаторов")]
    OrgList,
}

macro_rules! link {
    ($text:expr) => {
        link_impl($text, None)
    };
    ($text:expr, $url:expr) => {
        link_impl($text, Some($url))
    };
}

async fn help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();
    let token = &env::var("TOKEN").expect("TOKEN not set");
    let database_url = &env::var("DATABASE_URL").expect("DATABASE_URL not set");

    info!("Starting bot...");

    let bot = Bot::new(token);

    let pool = SqlitePool::connect(database_url).await.unwrap();
    let pool = Arc::new(pool); // Обернуть pool в Arc

    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            username TEXT,
            name TEXT NOT NULL,
            course TEXT NOT NULL,
            question TEXT NOT NULL
        )
        "#
    )
    .execute(&*pool)
    .await
    .unwrap();


    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<FormState>::new(), pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

// Функция для проверки, разрешён ли пользователь
fn is_admin(user_id: ChatId) -> bool {
    // info!("{}", ALLOWED_USER_IDS.contains(&user_id.0));

    // Загружаем переменную окружения
    let ids_str = env::var("ALLOWED_USER_IDS").expect("IDS not set in .env");

    // Разбиваем строку на массив ID
    let ids: Vec<i64> = ids_str
        .split(',')
        .map(|s| s.parse::<i64>().expect("Invalid ID format")) // Используем parse
        .collect();

    ids.contains(&user_id.0)
}

async fn start(bot: Bot, msg: Message, dialogue: MyDialogue, pool: Arc<SqlitePool>) -> HandlerResult {
    match find_user_by_id(pool, msg.chat.id.0).await {
        Ok(Some(user)) => {
            info!("User found: {:?}", user);
            bot.send_message(msg.chat.id, "Ты уже заполнил форму!").await?;
        },
        Ok(None) => {
            info!("User: {:?}", msg.chat.id.0);
            bot.send_message(msg.chat.id, "Привет!\n\nПеред тем как попасть в оргком поречья 46 ответь, пожалуйста, на несколько вопросов!\n\nКак тебя зовут?").await?;
            dialogue.update(FormState::ReceiveFullName).await?;
        },
        Err(e) => error!("Error: {}", e),
    }

    Ok(())
}

async fn cancel(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Cancelling the dialogue.").await?;
    dialogue.exit().await?;
    Ok(())
}

async fn orglist(bot: Bot, msg: Message, pool: Arc<SqlitePool>) -> HandlerResult {
    let users = sqlx::query_as!(
        User,
        r#"
        SELECT *
        FROM users 
        "#,
    )
    .fetch_all(&*pool)
    .await?;
    info!("Найдено организаторов: {}", users.len());
    bot.send_message(msg.chat.id, format!("Найдено организаторов: {}", users.len())).await?;

    // Создание CSV-файла
    let mut writer = Writer::from_path("output.csv")?;

    for user in users {
        writer.serialize(user)?;
    }

    writer.flush()?;

    println!("Данные успешно экспортированы в output.csv");

    bot.send_document(msg.chat.id, InputFile::file("output.csv"))
        .await?;
    Ok(())
}


async fn waiting_for_name(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            let keyboard = KeyboardMarkup::new(vec![
                vec![KeyboardButton::new("1"), KeyboardButton::new("2"), KeyboardButton::new("3")],
                vec![KeyboardButton::new("4"), KeyboardButton::new("5"), KeyboardButton::new("6+")],
            ]).one_time_keyboard();
    
            bot.send_message(msg.chat.id, "На каком ты курсе?").reply_markup(keyboard).await?;
            dialogue.update(FormState::ReceiveCourse { full_name: text.into() }).await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправь мне текст!").await?;
        }
    }

    Ok(())
}

async fn waiting_for_course(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    full_name: String, // Available from `State::ReceiveAge`.
    pool: Arc<SqlitePool>,
) -> HandlerResult {
    match msg.text() {
        Some(course) => {
            let course: String = course.into();
            if course == "6+" {
                bot.send_message(msg.chat.id, format!("Ого!\n\nВот это действительно взрослые люди решили подключиться к нам!\nРад видеть! Не буду томить, заходи в оргком — {}!", link!("добро пожаловать")))
                    .parse_mode(ParseMode::Html).await?;
                add_user(pool, msg.chat.id, msg.chat.username(), full_name, course.into(), "".into()).await;
                dialogue.exit().await?;
            } else if let Ok(course_i) = course.parse::<u8>() {
                if course_i > 4 {
                    bot.send_message(msg.chat.id, format!("Ого!\n\nВот это действительно взрослые люди решили подключиться к нам!\nРад видеть! Не буду томить, заходи в оргком — {}!", link!("добро пожаловать")))
                        .parse_mode(ParseMode::Html).await?;
                    add_user(pool, msg.chat.id, msg.chat.username(), full_name, course, "".into()).await;
                    dialogue.exit().await?;
                } else if (1..=4).contains(&course_i) {
                    bot.send_message(msg.chat.id, "Теперь самый важный вопрос!\n\nЗачем и почему ты хочешь делать поречье 46?\nПодумай немного и расскажи здесь!").reply_markup(ReplyMarkup::kb_remove()).await?;
                    dialogue.update(FormState::ReceiveQuestion { full_name: (full_name), course: (course) }).await?;
                } else {
                    bot.send_message(msg.chat.id, "Повторите ввод").reply_markup(ReplyMarkup::kb_remove()).await?;
                }
            } else {
                bot.send_message(msg.chat.id, "Повторите ввод (некорректный формат)").reply_markup(ReplyMarkup::kb_remove()).await?;
            }
        }
        None => {
            bot.send_message(msg.chat.id, "Повторите ввод").reply_markup(ReplyMarkup::kb_remove()).await?;
        }
    }

    Ok(())
}

async fn waiting_for_question(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    (full_name, course): (String, String), // Available from `State::ReceiveAge`.
    pool: Arc<SqlitePool>,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            bot.send_message(
                msg.chat.id, 
                format!("Круто!\n\nСпасибо, что рассказал, зачем и почему хочешь делать поречье 46. Оно будет уже через 2,5 месяца. Времени не очень много, поэтому можешь смело заходить в оргком — {}!", link!("добро пожаловать"))
            ).parse_mode(ParseMode::Html).await?;
            add_user(pool, msg.chat.id, msg.chat.username(), full_name, course, text.into()).await;
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправь мне текст!").await?;
        }
    }

    Ok(())
}

async fn add_user(
    pool: Arc<SqlitePool>,
    chat_id: ChatId,
    username: Option<&str>,
    name: String,
    course: String,
    question: String
) -> Result<(), sqlx::Error>  {
    sqlx::query!(
        r#"
        INSERT INTO users (chat_id, username, name, course, question)
        VALUES (?, ?, ?, ?, ?)
        "#,
        chat_id.0,
        username,
        name,
        course,
        question
    )
    .execute(&*pool)
    .await?;

    info!("User added: {:?}, {:?}, {:?}, {:?}, {:?}", chat_id, username, name, course, question);
    Ok(())
}

async fn find_user_by_id(pool: Arc<SqlitePool>, user_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let row= sqlx::query!(
        r#"
        SELECT chat_id
        FROM users
        WHERE chat_id = ?
        "#,
        user_id
    )
    .fetch_optional(&*pool)
    .await?;
    // Извлекаем chat_id из результата
    let chat_id = row.map(|r| r.chat_id);
    info!(chat_id);

    Ok(chat_id)
}

fn link_impl(text: &str, url: Option<&str>) -> String {
    let url = url.map(|u| u.to_string()).unwrap_or_else(|| {
        env::var("LINK").expect("LINK environment variable not set")
    });
    format!("<a href=\"{}\">{}</a>", url, text)
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(
            case![FormState::Start]
                .branch(case![Command::Help].endpoint(help))
                .branch(case![Command::Start].endpoint(start)),
        )
        .branch(case![Command::Cancel].endpoint(cancel));

        let admin_command_handler = teloxide::filter_command::<CommandAdmin, _>()
        .filter(|msg: Message| {
            if let Some(chat_id) = msg.chat_id() {
                is_admin(chat_id)
            } else {
                false
            }
        })
        .branch(case![CommandAdmin::OrgList].endpoint(orglist));
    let message_handler = Update::filter_message()
        .branch(admin_command_handler)
        .branch(command_handler)
        .branch(dptree::case![FormState::Start].endpoint(start))
        .branch(dptree::case![FormState::ReceiveFullName].endpoint(waiting_for_name))
        .branch(dptree::case![FormState::ReceiveCourse { full_name }].endpoint(waiting_for_course))
        .branch(
            dptree::case![FormState::ReceiveQuestion { full_name, course }]
            .endpoint(waiting_for_question)
        );

    enter::<Update, InMemStorage<FormState>, FormState, _>()
        .branch(message_handler)
}