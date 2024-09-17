-- Add migration script here

ALTER TABLE build ADD COLUMN reason VARCHAR NOT NULL DEFAULT "unknown";