mod models;
mod config;

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::error::Error;
use std::path::Path;
use rusqlite::{Connection, Result};
use crate::models::{BaseOptions, LogRecord};

extern crate chrono;

use chrono::prelude::DateTime;
use chrono::Utc;
use std::time::{UNIX_EPOCH, Duration};
use config::{ELASTIC_URL, ELASTIC_LOGIN, ELASTIC_PASSWORD};

#[tokio::main]
async fn main() {
    let _ = tokio::spawn(task()).await.unwrap();
}

pub async fn task() {
    let result = do_task().await;
    match result {
        Ok(_) => {}
        Err(e) => println!("[ERROR] {:?}", e)
    };
}

async fn do_task() -> Result<(), Box<dyn Error>> {
    let base_options_list = read_base_options_file("collect_logs_params.txt")?;
    println!("{:#?}", base_options_list);

    for base_options in base_options_list {
        if Path::new(base_options.log.as_str()).exists() {
            let connection = open_connection(base_options.log.as_str())?;
            let logs = read_log_records(&connection, base_options.start_log_record, base_options.end_log_record, base_options.server.as_str(), base_options.name.as_str())?;
            let _ = save_logs_to_elastic(logs).await;
        }
    }
    Ok(())
}

async fn save_logs_to_elastic(logs: Vec<LogRecord>) -> Result<(), Box<dyn Error>> {
    let mut text: String = "".to_string();
    for log in logs {
        let json = serde_json::to_string_pretty(&log)?;
        let table = "utp_logs-2022.08";

        let client = reqwest::Client::new();
        let url = format!("{}/{}/_doc/{}", ELASTIC_URL, table, log.id);
        let response = client.post(url)
            .basic_auth(ELASTIC_LOGIN, Some(ELASTIC_PASSWORD))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        if status != 200 {
            text.push_str(format!("Error: {} : {} : {} : {}\n{}\n", status, log.id, log.database, log.date, body).as_str());
        }
    }

    if !text.is_empty() {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("collect_logs_errors.txt")?;
        file.write_all(text.as_bytes())?;
    }

    Ok(())
}

fn read_base_options_file(path: &str) -> Result<Vec<BaseOptions>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut list: Vec<BaseOptions> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let mut lines = line.split("|");
        lines.next();
        let base_options = BaseOptions {
            server: lines.next().unwrap().trim().to_string(),
            name: lines.next().unwrap().trim().to_string(),
            start_log_record: lines.next().unwrap().parse().unwrap(),
            end_log_record: lines.next().unwrap().parse().unwrap(),
            log: lines.next().unwrap().trim().to_string(),
        };
        list.push(base_options);
    }

    Ok(list)
}

fn open_connection(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    Ok(conn)
}

fn read_log_records(connection: &Connection, start_log_record: i64, end_log_record: i64, server: &str, database: &str) -> Result<Vec<LogRecord>> {
    let mut stmt = connection.prepare(
        r#"
        WITH EventNames(code, name) AS
          (
        SELECT '"_$Session$_.Start"', 'Сеанс. Начало'
        union all select '"_$Session$_.Authentication"', 'Сеанс. Аутентификация'
        union all select '"_$Session$_.Finish"', 'Сеанс. Завершение'
        union all select '"_$InfoBase$_.ConfigUpdate"', 'Информационная база. Изменение конфигурации'
        union all select '"_$InfoBase$_.DBConfigUpdate"', 'Информационная база. Изменение конфигурации базы данных'
        union all select '"_$InfoBase$_.EventLogSettingsUpdate"', 'Информационная база. Изменение параметров журнала регистрации'
        union all select '"_$InfoBase$_.InfoBaseAdmParamsUpdate"', 'Информационная база. Изменение параметров информационной базы'
        union all select '"_$InfoBase$_.MasterNodeUpdate"', 'Информационная база. Изменение главного узла'
        union all select '"_$InfoBase$_.RegionalSettingsUpdate"', 'Информационная база. Изменение региональных установок'
        union all select '"_$InfoBase$_.TARInfo"', 'Тестирование и исправление. Сообщение'
        union all select '"_$InfoBase$_.TARMess"', 'Тестирование и исправление. Предупреждение'
        union all select '"_$InfoBase$_.TARImportant"', 'Тестирование и исправление. Ошибка'
        union all select '"_$Data$_.New"', 'Данные. Добавление'
        union all select '"_$Data$_.Update"', 'Данные. Изменение'
        union all select '"_$Data$_.Delete"', 'Данные. Удаление'
        union all select '"_$Data$_.TotalsPeriodUpdate"', 'Данные. Изменение периода рассчитанных итогов'
        union all select '"_$Data$_.Post"', 'Данные. Проведение'
        union all select '"_$Data$_.Unpost"', 'Данные. Отмена проведения'
        union all select '"_$User$_.New"', 'Пользователи. Добавление'
        union all select '"_$User$_.Update"', 'Пользователи. Изменение'
        union all select '"_$User$_.Delete"', 'Пользователи. Удаление'
        union all select '"_$Job$_.Start"', 'Фоновое задание. Запуск'
        union all select '"_$Job$_.Succeed"', 'Фоновое задание. Успешное завершение'
        union all select '"_$Job$_.Fail"', 'Фоновое задание. Ошибка выполнения'
        union all select '"_$Job$_.Cancel"', 'Фоновое задание. Отмена'
        union all select '"_$PerformError$_"', 'Ошибка выполнения'
        union all select '"_$Transaction$_.Begin"', 'Транзакция. Начало'
        union all select '"_$Transaction$_.Commit"', 'Транзакция. Фиксация'
        union all select '"_$Transaction$_.Rollback"', 'Транзакция. Отмена'
    )
    SELECT
    CASE
			WHEN en.name LIKE '%Ошибка%' THEN 1 ELSE 0
		END as error,
    date as id,
    u.name AS user,
    c.name AS comp,
    a.name AS app,
    en.name AS event,
    comment,
    e.transactionID AS transaction_id,
    CASE
      WHEN e.transactionStatus = 1 THEN 'Зафиксирована'
      WHEN e.transactionStatus = 2 THEN 'Отменена'
      ELSE ''
    END as transaction_status,
    m.name as metadata,
    e.dataPresentation as data
    FROM
    EventLog e
    LEFT JOIN UserCodes u ON e.userCode = u.code
    LEFT JOIN ComputerCodes c ON e.computerCode = c.code
    LEFT JOIN AppCodes a ON e.appCode = a.code
    LEFT JOIN EventCodes ec ON e.eventCode = ec.code
    LEFT JOIN EventNames en ON ec.name = en.code
    LEFT JOIN MetadataCodes m ON e.metadataCodes = m.code
    WHERE
    (date >= ? AND date <= ?)
    order by date desc;"#,
    )?;

    let rows: Vec<Result<LogRecord>> = stmt.query_map([start_log_record, end_log_record], |row| {
        let is_error: bool = row.get(0)?;
        let transaction_status: String = row.get(8)?;
        let status: String = if is_error { "Ошибка".to_string() } else { transaction_status.clone() };
        let error: bool = is_error;
        let id: i64 = row.get(1)?;
        let user: String = row.get(2)?;
        let comp: String = match row.get(3)? {
            Some(value) => value,
            None => "".to_string()
        };
        let app: String = row.get(4)?;
        let event: String = row.get(5)?;
        let comment: String = row.get(6)?;
        let transaction_id: i64 = row.get(7)?;
        let metadata: String = match row.get(9)? {
            Some(value) => value,
            None => "".to_string()
        };
        let data: String = row.get(10)?;
        let date: DateTime<Utc> = id_to_date_time(id);
        let server: String = server.to_string();
        let database: String = database.to_string();

        Ok(LogRecord {
            error,
            id,
            user,
            comp,
            app,
            event,
            comment,
            transaction_id,
            transaction_status,
            metadata,
            data,
            status,
            date,
            server,
            database,
        })
    })?.collect();

    let mut records: Vec<LogRecord> = vec![];
    for row in rows {
        records.push(row?);
    }

    Ok(records)
}

fn id_to_date_time(id: i64) -> DateTime<Utc> {
    const MILLISECONDS_FROM_AD_TO_EPOCH: i64 = 62135596800000;
    let duration = UNIX_EPOCH + Duration::from_millis((id / 10 - MILLISECONDS_FROM_AD_TO_EPOCH) as u64);
    DateTime::<Utc>::from(duration)
}