DROP TABLE IF EXISTS data_all CASCADE;

CREATE TABLE data_all (
    band_index  INT PRIMARY KEY,
    band_name   TEXT,
    country     TEXT,
    genre       TEXT,
    status      TEXT
);

COPY data_all
FROM '/data/metal_bands.csv'
WITH (FORMAT csv, HEADER true);

DROP TABLE IF EXISTS data_0;

CREATE TABLE data_0 (
    band_index  INT PRIMARY KEY,
    band_name   TEXT,
    country     TEXT,
    genre       TEXT,
    status      TEXT
);

INSERT INTO data_0
SELECT
    band_index,
    band_name,
    country,
    genre,
    status
FROM data_all
ORDER BY band_index ASC
LIMIT 1;

DO $$
DECLARE
    i INT;
BEGIN
    FOR i IN 10..17 LOOP

        EXECUTE format('DROP TABLE IF EXISTS data_%s;', i);

        EXECUTE format(
            'CREATE TABLE data_%s (
                band_index  INT PRIMARY KEY,
                band_name   TEXT,
                country     TEXT,
                genre       TEXT,
                status      TEXT
             );',
            i
        );

        EXECUTE format(
            'INSERT INTO data_%s
             SELECT
                band_index,
                band_name,
                country,
                genre,
                status
             FROM data_all
             ORDER BY band_index ASC
             LIMIT %s;',
            i,
            (2^i)::INT
        );

    END LOOP;
END $$;
