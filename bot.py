from aiogram import Bot, Dispatcher, F
from aiogram.types import Message, KeyboardButton, ReplyKeyboardMarkup, ReplyKeyboardRemove

from aiogram.filters import Command
from aiogram.fsm.context import FSMContext
from aiogram.fsm.state import State, StatesGroup

from table import add_user_async, init_db
from logger import logger
from settings import TOKEN, LINK

bot = Bot(TOKEN)
dp = Dispatcher()

FINAL = f"""
круто!
спасибо, что рассказал, зачем и почему хочешь делать поречье 46. оно будет уже через 2,5 месяца. времени не очень много, поэтому можешь смело заходить в оргком — <a href="{LINK}">добро пожаловать</a>!
"""
FINAL_ALT = f"""
ого!

вот это действительно взрослые люди решили подключиться к нам!
рад видеть! не буду томить, заходи в оргком — <a href="{LINK}">добро пожаловать</a>!
"""

class Form(StatesGroup):
    waiting_for_name = State()
    waiting_for_course = State()
    waiting_for_question = State()

kb = ReplyKeyboardMarkup(
    keyboard=[
        [KeyboardButton(text="1"), KeyboardButton(text="2"), KeyboardButton(text="3")],
        [KeyboardButton(text="4"), KeyboardButton(text="5"), KeyboardButton(text="6+")]
    ],
    one_time_keyboard=True
)
remove_kb = ReplyKeyboardRemove()

@dp.message(Command("start"))
async def waiting_for_course(message: Message, state: FSMContext):
    await message.answer("""
привет!

перед тем как попасть в оргком поречья 46 ответь, пожалуйста, на несколько вопросов!

как тебя зовут?
""")
    await state.set_state(Form.waiting_for_name)

@dp.message(F.text, Form.waiting_for_name)
async def password_handler(message: Message, state: FSMContext):
    await message.answer("на каком ты курсе?", reply_markup=kb)

    await state.update_data(waiting_for_name=message.text)
    await state.set_state(Form.waiting_for_course)

@dp.message(F.text, Form.waiting_for_course)
async def course_handler(message: Message, state: FSMContext):
    course_text = message.text
    await state.update_data(waiting_for_course=course_text)

    async def process_user(data):
        await message.answer(FINAL_ALT, parse_mode="html", reply_markup=remove_kb)
        await add_user_async([
            message.from_user.id, 
            message.from_user.username, 
            data["waiting_for_name"], 
            data["waiting_for_course"], 
        ], worksheet)
        del data
        await state.clear()

    if course_text == "6+":
        data = await state.get_data()
        await process_user(data)
        return

    try:
        course = int(course_text)
        if course > 4:
            data = await state.get_data()
            await process_user(data)
        elif 1 <= course <= 4:
            await message.answer("""
теперь самый важный вопрос!

зачем и почему ты хочешь делать поречье 46? 
подумай немного и расскажи здесь!""", reply_markup=remove_kb)
            await state.set_state(Form.waiting_for_question)
        else:
            await message.answer("Повторите ввод", reply_markup=remove_kb)

    except ValueError:
        await message.answer("Повторите ввод (некорректный формат)", reply_markup=remove_kb)

@dp.message(F.text, Form.waiting_for_question)
async def password_handler(message: Message, state: FSMContext):
    await message.answer(FINAL, parse_mode="html")
    
    await state.update_data(waiting_for_question=message.text)
    # добавить в таблицу
    data = await state.get_data()

    await add_user_async([
        message.from_user.id, 
        message.from_user.username, 
        data["waiting_for_name"], 
        data["waiting_for_course"], 
        data["waiting_for_question"]
        ], worksheet)
    await state.clear()

async def main():
    global worksheet
    worksheet = await init_db()
    await dp.start_polling(bot, skip_updates=True)

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
