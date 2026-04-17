# SimplePIR
Project for my Matersthesis.

## Running the tests

The integration tests read from Postgres. From the project root, start the database first:

```bash
docker compose up db
```

The tests default to:

```bash
DATABASE_URL="host=localhost user=user password=password dbname=pir_db"
```

Run all integration tests:

```bash
cargo test -p evaluation --tests
```

Run each integration test individually:

```bash
cargo test -p evaluation --test test_single_query_integration
cargo test -p evaluation --test test_batch_query_integration
cargo test -p evaluation --test test_keyword_mapping_integration
```
For the keyword search integration test, think of a keyword to search for (e.g. "Metallica") in the database and set it as an environment variable before running the test:

```bash
KEYWORD=Metallica cargo test -p evaluation --test test_keyword_search_integration
```

