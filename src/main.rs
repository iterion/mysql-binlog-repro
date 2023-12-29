use anyhow::Result;
use mysql_async::{
    prelude::{Query, WithParams},
    binlog::row::BinlogRow,
};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let url = "mysql://root:@localhost:3306/main";
    let opts = mysql_async::Opts::from_url(&url)?;
    let pool = mysql_async::Pool::new(opts);

    let mut conn = pool.get_conn().await?;

    // Let's get our current position in the binlog.
    let binlog_positions = "SHOW BINARY LOGS"
        .with(())
        .map(&mut conn, |(filename, position, encrypted)| {
            BinLogPosition {
                filename,
                position,
                encrypted,
            }
        })
        .await?;

    // We want the last one.
    if binlog_positions.is_empty() {
        anyhow::bail!("No binlogs found");
    }

    let last_position = binlog_positions.last().unwrap();

    // Now let's get the bin log stream.
    let mut binlog_req = mysql_async::BinlogStreamRequest::new(conn.id());
    // We want to set our position to the latest ones.
    binlog_req = binlog_req.with_filename(last_position.filename.as_bytes());
    binlog_req = binlog_req.with_pos(last_position.position as u64);

    let mut binlog_stream = conn.get_binlog_stream(binlog_req).await?;
    while let Some(event) = binlog_stream.next().await {
        if let Err(err) = handle_binlog_event(event?, &binlog_stream).await {
            println!("error handling binlog event {err:?}");
        }
    }

    Ok(())
}

struct UuidExample {
    id: uuid::Uuid,
}
impl UuidExample {
    fn convert(row: &mysql_async::binlog::row::BinlogRow) -> Result<Self> {
        let mut example = UuidExample {
            // @0
            id: Default::default(),
        };

        let columns = row.columns_ref();
        for (index, column) in columns.iter().enumerate() {
            let value = row
                .as_ref(index)
                .ok_or_else(|| anyhow::anyhow!("binlog value was none!"))?;

            let column_name = column.name_str();
            if column_name == "@0" {
                example.id = value_as_uuid(value)?;
            }
        }

        Ok(example)
    }
}

#[derive(Debug)]
pub struct BinLogPosition {
    pub filename: String,
    pub position: i64,
    pub encrypted: String,
}

async fn handle_binlog_event(
    event: mysql_async::binlog::events::Event,
    binlog_stream: &mysql_async::BinlogStream,
) -> Result<()> {
    let data = event.read_data()?;
    // We only care about new events for certain things.
    if let Some(mysql_async::binlog::events::EventData::RowsEvent(rows_event)) = &data {
        // We need to figure out the table name.
        let table_map = binlog_stream
            .get_tme(rows_event.table_id())
            .ok_or_else(|| anyhow::anyhow!("Could not get table map for event"))?;
        let table_name = table_map.table_name();
        if table_name == "heartbeat" {
            // We don't care about heartbeats.
            return Ok(());
        }
        for row in rows_event.rows(table_map) {
            let (_, row) = row?;
            let row = row.ok_or_else(|| anyhow::anyhow!("binlog row was none!"))?;

            let uuid_example = UuidExample::convert(&row)?;
        }
    }

    Ok(())
}


fn value_as_uuid(
    value: &mysql_async::binlog::value::BinlogValue,
) -> Result<uuid::Uuid> {
    if let mysql_async::binlog::value::BinlogValue::Value(mysql_async::Value::Bytes(b)) = value {
        let uuid = match uuid::Uuid::from_slice(&b[..]) {
            Ok(uuid) => uuid,
            Err(e) => return Err(anyhow::anyhow!("Failed to parse {b:?}: {e:?}")),
        };
        Ok(uuid)
    } else {
        Err(anyhow::anyhow!("binlog value was not a uuid: {:?}", value))
    }
}
