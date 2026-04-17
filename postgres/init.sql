DROP TABLE IF EXISTS data_all CASCADE;

CREATE TABLE data_all (
    band_index  INT PRIMARY KEY,
    band_name   TEXT,
    fans        TEXT, 
    formed      TEXT, 
    origin      TEXT,
    split       TEXT,
    style       TEXT
);

COPY data_all 
FROM '/data/metal_bands.csv' 
WITH (FORMAT csv, HEADER true);

UPDATE data_all 
SET fans = '0' 
WHERE fans = '-' OR fans IS NULL;

UPDATE data_all 
SET formed = '0' 
WHERE formed = '-' OR formed IS NULL;

UPDATE data_all 
SET split = '0' 
WHERE split = '-' OR split IS NULL;

DROP TABLE IF EXISTS data_0;

CREATE TABLE data_0 AS
SELECT 
    band_index, 
    band_name, 
    fans::INT,
    formed::INT,
    origin,
    split::INT,
    style
FROM data_all 
ORDER BY band_index ASC 
LIMIT 1;

DO $$
DECLARE
    i INT;
BEGIN
    FOR i IN 10..16 LOOP

        EXECUTE format('DROP TABLE IF EXISTS data_%s;', i);

        EXECUTE format(
            'CREATE TABLE data_%s AS 
             SELECT 
                band_index, 
                band_name, 
                fans::INT,
                formed::INT,
                origin,
                split::INT,
                style
             FROM data_all
             ORDER BY band_index ASC
             LIMIT %s;',
            i,
            (2^i)::INT
        );

    END LOOP;
END $$;