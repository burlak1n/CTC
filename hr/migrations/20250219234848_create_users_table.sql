-- Add migration script here
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    username TEXT,
    name TEXT NOT NULL,
    course TEXT NOT NULL,
    question TEXT,
    mailing BOOLEAN NOT NULL DEFAULT TRUE
);