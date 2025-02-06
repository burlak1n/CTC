import asyncio

from gspread import service_account, Client
from loguru import logger
from settings import TABLE_LINK

def client_init_json() -> Client:
    """Создание клиента для работы с Google Sheets."""
    return service_account(filename='./data/loader_test.json')

def get_table_by_url(client: Client, table_url):
    """Получение таблицы из Google Sheets по ссылке."""
    return client.open_by_url(table_url)

async def init_db():
    client = client_init_json()
    table = get_table_by_url(client, TABLE_LINK)
    return table.worksheet("Бот")

def add_user(row, worksheet):
    worksheet.append_row(row)
    logger.success(f"Пользователь успешно добавлен: {row}")

async def add_user_async(row, worksheet):
    """
    Асинхронно запускает синхронную функцию add_row_to_sheet_sync в отдельном потоке.
    """
    loop = asyncio.get_event_loop()
    await loop.run_in_executor(None, add_user, row, worksheet)

