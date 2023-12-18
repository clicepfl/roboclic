CREATE TABLE committee(
    id SERIAL PRIMARY KEY,
    first_name VARCHAR(50) NOT NULL,
    surname VARCHAR(50) NOT NULL,
    telegram_id VARCHAR(50),
    poll_count INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE admins(
    telegram_id VARCHAR(50) PRIMARY KEY,
    "name" VARCHAR(200) NOT NULL
);
CREATE TABLE authorizations(
    command VARCHAR(50) NOT NULL,
    chat_id VARCHAR(50) NOT NULL
);