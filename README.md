# mysql-binlog-repro

Set up the DB, example schema in setup.sql. Url is hardcoded in `src/main.rs` for now.

I used the versions we reproduced on. But, I realize now we're a few versions behind on mysql_async too, so I'm going to update this example to the latest version and see if it still happens.

Run the rust binary:

```
cargo run
```

Now insert a row with trailing 0s:

```sql
INSERT INTO uuid_examples (id) VALUES (UUID_TO_BIN('dc23a9b9-a129-11ee-95fb-0242ac110000'));
```

See the resultant error:

```
error handling binlog event Failed to parse [220, 35, 169, 185, 161, 41, 17, 238, 149, 251, 2, 66, 172, 17]: Error(ByteLength { len: 14 })
```
