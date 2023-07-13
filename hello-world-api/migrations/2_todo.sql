create table "todo"
(
    id          uuid primary key default gen_random_uuid(),
    todo_text   text unique not null,
    is_done     BOOLEAN NOT NULL DEFAULT FALSE
);