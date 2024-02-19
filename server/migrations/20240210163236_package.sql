CREATE TABLE IF NOT EXISTS package
(
    base     VARCHAR  NOT NULL PRIMARY KEY,
    -- utc
    added    DATETIME NOT NULL,

    -- json
    source   VARCHAR  NOT NULL,

    -- parsable srcinfo
    srcinfo  VARCHAR,
    pkgbuild VARCHAR,

    -- actual version, may be different from srcinfo
    version  VARCHAR,

    enabled  BOOLEAN  NOT NULL,
    clean    BOOLEAN  NOT NULL,

    schedule VARCHAR,
    prepare  VARCHAR
)
