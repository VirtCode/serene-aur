CREATE TABLE IF NOT EXISTS build
(
    package     VARCHAR  NOT NULL,

    started     DATETIME NOT NULL,
    ended       DATETIME,

    state       VARCHAR  NOT NULL,
    progress    VARCHAR,

    -- error message if present
    fatal       VARCHAR,

    version     VARCHAR,

    -- run information, like the actual build
    run_success BOOLEAN,
    run_logs    VARCHAR,
    run_started DATETIME,
    run_ended   DATETIME,

    PRIMARY KEY (package, started),
    FOREIGN KEY (package) REFERENCES package(base) ON DELETE CASCADE
)