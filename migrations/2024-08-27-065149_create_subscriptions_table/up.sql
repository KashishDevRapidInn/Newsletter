CREATE TABLE subscriptions(
    id uuid PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL, 
    subscribed_at TIMESTAMPTZ NOT NULL
);
