use std::collections::HashSet;

use chrono::NaiveDate;
use serde_json::{json, Value};

use crate::api::Period;
use crate::config::Config;
use crate::dates::{from_api, ymd};
use crate::error::{msg, Result};

pub const WORK_DAY: i64 = 1;

#[derive(Clone, Debug)]
pub struct Day {
    pub api: String,
    pub date: NaiveDate,
    pub day_id: i64,
    pub day_type: String,
    pub non_wrk: bool,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub period: Period,
    pub emp: Value,
    pub days: Vec<Value>,
    pub grid: Vec<Value>,
    pub today: NaiveDate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Past,
    Current,
    Future,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Fill,
    Submit,
}

impl Action {
    pub fn command(self) -> &'static str {
        match self {
            Action::Fill => "timesheet fill",
            Action::Submit => "timesheet submit",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct State {
    pub label: &'static str,
    pub color: Option<&'static str>,
    pub action: Option<Action>,
}

#[derive(Debug)]
pub struct Summary {
    pub data: Data,
    pub info: Vec<Day>,
    pub filled: usize,
    pub total: usize,
    pub missing_now: Vec<Day>,
    pub missing_future: Vec<Day>,
    pub complete: bool,
    pub last_workday: Option<NaiveDate>,
    pub kind: Kind,
    pub editable: bool,
    pub is_new: bool,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub entries: Vec<Value>,
    pub targets: Vec<Day>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlanOptions {
    pub only_date: Option<NaiveDate>,
    pub force: bool,
    pub noop: bool,
    pub allow_new: bool,
}

impl Summary {
    pub fn new(data: Data) -> Self {
        let info = normalize_days(&data.days);
        let workdays: Vec<Day> = info
            .iter()
            .filter(|day| day.day_id == WORK_DAY && !day.non_wrk)
            .cloned()
            .collect();
        let today = data.today;
        let tracked: Vec<(Day, f64)> = workdays
            .iter()
            .map(|day| (day.clone(), hours_on(&data.grid, &day.api)))
            .collect();

        let filled = tracked.iter().filter(|(_, hours)| *hours > 0.0).count();
        let mut missing_now = Vec::new();
        let mut missing_future = Vec::new();
        for (day, hours) in &tracked {
            if *hours == 0.0 {
                if day.date <= today {
                    missing_now.push(day.clone());
                } else {
                    missing_future.push(day.clone());
                }
            }
        }
        let complete = !tracked.is_empty() && tracked.iter().all(|(_, hours)| *hours > 0.0);
        let kind = if data.period.end < today {
            Kind::Past
        } else if data.period.start > today {
            Kind::Future
        } else {
            Kind::Current
        };

        let status = text(&data.emp, "Status");
        let editable = status == "0" || status == "1";
        let is_new = !data.grid.is_empty() && data.grid.iter().all(|row| number(row, "TID") <= 0);
        let last_workday = workdays.last().map(|day| day.date);

        Self {
            total: tracked.len(),
            info,
            filled,
            missing_now,
            missing_future,
            complete,
            last_workday,
            kind,
            editable,
            is_new,
            data,
        }
    }

    pub fn hours_on(&self, api: &str) -> f64 {
        hours_on(&self.data.grid, api)
    }
}

pub fn normalize_days(days: &[Value]) -> Vec<Day> {
    let mut info: Vec<Day> = days
        .iter()
        .filter_map(|row| {
            let api = row.get("StartDate")?.as_str()?.trim().to_string();
            let date = from_api(&api).ok()?;
            Some(Day {
                api,
                date,
                day_id: number(row, "DayID"),
                day_type: text(row, "DayType"),
                non_wrk: flag(row.get("EnblNonWRKDay")),
            })
        })
        .collect();
    info.sort_by_key(|day| day.date);
    info
}

pub fn hours_on(grid: &[Value], api: &str) -> f64 {
    grid.iter()
        .map(|row| {
            row.get("lstColumns")
                .and_then(|v| v.as_array())
                .and_then(|columns| {
                    columns
                        .iter()
                        .find(|column| column.get("Key").and_then(|k| k.as_str()) == Some(api))
                })
                .and_then(|column| column.get("Value"))
                .map(|value| {
                    value
                        .as_f64()
                        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
                        .unwrap_or(0.0)
                })
                .unwrap_or(0.0)
        })
        .sum()
}

pub fn state_of(summary: &Summary) -> State {
    if summary.kind == Kind::Future {
        return State {
            label: "futuro",
            color: Some("2"),
            action: None,
        };
    }
    if !summary.editable {
        let done = text(&summary.data.emp, "StatusDesc").to_lowercase();
        let done = done.contains("complet");
        return State {
            label: if done { "aprobado" } else { "enviado" },
            color: Some("2"),
            action: None,
        };
    }
    if summary.kind == Kind::Past {
        return if summary.complete {
            State {
                label: "falta submit",
                color: Some("33"),
                action: Some(Action::Submit),
            }
        } else {
            State {
                label: "atrasado",
                color: Some("31"),
                action: Some(Action::Fill),
            }
        };
    }
    if summary.complete
        && summary
            .last_workday
            .is_some_and(|day| summary.data.today >= day)
    {
        return State {
            label: "falta submit",
            color: Some("33"),
            action: Some(Action::Submit),
        };
    }
    State {
        label: "en curso",
        color: None,
        action: if summary.missing_now.is_empty() {
            None
        } else {
            Some(Action::Fill)
        },
    }
}

pub fn submit_due(summary: &Summary) -> bool {
    summary.complete
        && (summary.kind == Kind::Past || summary.last_workday == Some(summary.data.today))
}

pub fn plan(config: &Config, data: &Data, options: &PlanOptions) -> Result<Plan> {
    let info = normalize_days(&data.days);
    let hint = format!("Ejecuta: timesheet fill {}", ymd(data.period.start));

    let all_rows: Vec<&Value> = data.grid.iter().filter(|row| row.is_object()).collect();
    if all_rows.is_empty() {
        return Err(msg(
            "El periodo no tiene filas de proyecto. Abre el timesheet una vez en la web y reintenta.",
        ));
    }

    let pending: Vec<&Value> = all_rows
        .iter()
        .copied()
        .filter(|row| number(row, "TID") <= 0)
        .collect();
    let saved: Vec<&Value> = all_rows
        .iter()
        .copied()
        .filter(|row| number(row, "TID") > 0)
        .collect();

    let mut rows = saved;
    if rows.is_empty() {
        let pending_primary = pending
            .iter()
            .copied()
            .find(|row| text(row, "ProjectCode") == config.project_code);
        match pending_primary {
            Some(row) if options.allow_new => rows = vec![row],
            _ => {
                let found: Vec<String> = pending
                    .iter()
                    .map(|row| text(row, "ProjectCode"))
                    .filter(|code| !code.is_empty())
                    .collect();
                let found = if found.is_empty() {
                    "sin proyecto".to_string()
                } else {
                    found.join(", ")
                };
                return Err(msg(format!(
                    "El periodo aún no está inicializado (filas con TID=0: {found}). {hint}"
                )));
            }
        }
    }

    let primary = match rows
        .iter()
        .copied()
        .find(|row| text(row, "ProjectCode") == config.project_code)
    {
        Some(row) => row,
        None => {
            if pending
                .iter()
                .any(|row| text(row, "ProjectCode") == config.project_code)
            {
                return Err(msg(format!(
                    "La fila del proyecto {} aún no está inicializada (TID=0). {hint}",
                    config.project_code
                )));
            }
            let found: Vec<String> = rows
                .iter()
                .map(|row| text(row, "ProjectCode"))
                .filter(|code| !code.is_empty())
                .collect();
            return Err(msg(format!(
                "No hay fila para el proyecto {}. Filas del periodo: {}",
                config.project_code,
                if found.is_empty() {
                    "ninguno".to_string()
                } else {
                    found.join(", ")
                }
            )));
        }
    };

    let is_past = data.period.end < data.today;
    let workdays: Vec<Day> = info
        .iter()
        .filter(|day| day.day_id == WORK_DAY && !day.non_wrk)
        .cloned()
        .collect();

    let mut targets: Vec<Day> = if options.noop {
        Vec::new()
    } else {
        workdays
            .iter()
            .filter(|day| is_past || day.date <= data.today)
            .cloned()
            .collect()
    };

    if let Some(only) = options.only_date {
        targets.retain(|day| day.date == only);
        if targets.is_empty() {
            return Err(msg(format!(
                "{} no es un día laborable pendiente dentro del periodo.",
                ymd(only)
            )));
        }
    }
    if !options.force {
        targets.retain(|day| hours_on(&data.grid, &day.api) == 0.0);
    }
    let target_set: HashSet<&str> = targets.iter().map(|day| day.api.as_str()).collect();

    let mut entries = Vec::new();
    for row in &rows {
        let columns = row.get("lstColumns").and_then(|v| v.as_array());
        for day in &info {
            let column = columns.and_then(|columns| {
                columns.iter().find(|column| {
                    column.get("Key").and_then(|k| k.as_str()) == Some(day.api.as_str())
                })
            });
            let is_target = std::ptr::eq(*row, primary) && target_set.contains(day.api.as_str());

            entries.push(json!({
                "TID": or_default(row.get("TID"), json!(0)),
                "EmpCode": or_default(data.emp.get("EmpCode"), json!("")),
                "TimesheetFromDate": data.period.start_api,
                "TimesheetToDate": data.period.end_api,
                "ProjectCode": if is_target { json!(config.project_code) } else { or_default(row.get("ProjectCode"), json!("")) },
                "ActivityCode": if is_target { json!(config.activity_code) } else { or_default(row.get("ActivityCode"), json!("")) },
                "CategoryCode": if is_target { json!(config.category_code) } else { or_default(row.get("CategoryCode"), json!("")) },
                "ShiftCode": if is_target { json!(config.shift_code) } else { or_default(row.get("ShiftCode"), json!("")) },
                "SupervisorCode": or_default(data.emp.get("SupervisorCode"), json!("")),
                "IsMultipleSupv": bool_text(data.emp.get("IsMultipleSupv")),
                "TimesheetDate": day.api,
                "EffortHrs": if is_target { format!("{:.2}", config.hours_per_day) } else { value_string(column.and_then(|column| column.get("Value"))) },
                "DayID": day.day_id,
                "DID": or_default(column.and_then(|column| column.get("DID")), json!(0)),
                "Remarks1": value_string(column.and_then(|column| column.get("Remarks1"))),
            }));
        }
    }

    Ok(Plan { entries, targets })
}

fn flag(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_i64() == Some(1),
        Some(Value::String(value)) => {
            value.trim() == "1" || value.trim().eq_ignore_ascii_case("true")
        }
        _ => false,
    }
}

fn or_default(value: Option<&Value>, fallback: Value) -> Value {
    match value {
        Some(Value::Null) | None => fallback,
        Some(value) => value.clone(),
    }
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        None | Some(Value::Null) => String::new(),
        Some(Value::Number(value)) => match value.as_i64() {
            Some(value) => value.to_string(),
            None => value.as_f64().map(number_text).unwrap_or_default(),
        },
        Some(value) => value.to_string(),
    }
}

fn number_text(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(|value| value_string(Some(value)))
        .unwrap_or_default()
}

fn number(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .map(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

fn bool_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => "false".to_string(),
        Some(value) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn period() -> Period {
        Period {
            start_api: "02-Nov-2026".into(),
            end_api: "03-Nov-2026".into(),
            start: date(2026, 11, 2),
            end: date(2026, 11, 3),
        }
    }

    fn data(today: NaiveDate) -> Data {
        Data {
            period: period(),
            emp: json!({
                "EmpCode": "123",
                "Status": "1",
                "StatusDesc": "Pending",
                "SupervisorCode": "9",
                "IsMultipleSupv": false
            }),
            days: vec![
                json!({"StartDate": "02-Nov-2026", "DayID": 1, "DayType": "Working", "EnblNonWRKDay": "0"}),
                json!({"StartDate": "03-Nov-2026", "DayID": 1, "DayType": "Working", "EnblNonWRKDay": "0"}),
            ],
            grid: vec![json!({
                "TID": 55,
                "ProjectCode": "P1",
                "ActivityCode": "1",
                "CategoryCode": "1",
                "ShiftCode": "0",
                "lstColumns": [
                    {"Key": "02-Nov-2026", "Value": "8", "DID": 1, "Remarks1": ""},
                    {"Key": "03-Nov-2026", "Value": "", "DID": 2, "Remarks1": ""}
                ]
            })],
            today,
        }
    }

    fn config() -> Config {
        Config {
            emp_code: "123".into(),
            project_code: "P1".into(),
            ..Config::default()
        }
    }

    #[test]
    fn summarizes_current_period() {
        let summary = Summary::new(data(date(2026, 11, 3)));
        assert_eq!(summary.total, 2);
        assert_eq!(summary.filled, 1);
        assert!(!summary.complete);
        assert_eq!(summary.missing_now.len(), 1);
        assert!(summary.missing_future.is_empty());
        assert_eq!(state_of(&summary).action, Some(Action::Fill));
    }

    #[test]
    fn plans_missing_days() {
        let data = data(date(2026, 11, 3));
        let plan = plan(
            &config(),
            &data,
            &PlanOptions {
                allow_new: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(plan.targets.len(), 1);
        assert_eq!(plan.targets[0].api, "03-Nov-2026");
        assert_eq!(plan.entries.len(), 2);

        let target = plan
            .entries
            .iter()
            .find(|entry| entry["TimesheetDate"] == "03-Nov-2026")
            .unwrap();
        assert_eq!(target["EffortHrs"], "8.00");
        assert_eq!(target["DID"], 2);
        assert_eq!(target["ProjectCode"], "P1");

        let untouched = plan
            .entries
            .iter()
            .find(|entry| entry["TimesheetDate"] == "02-Nov-2026")
            .unwrap();
        assert_eq!(untouched["EffortHrs"], "8");
    }

    #[test]
    fn past_complete_needs_submit() {
        let mut data = data(date(2026, 11, 10));
        data.grid[0]["lstColumns"][1]["Value"] = json!("8");
        let summary = Summary::new(data);
        assert!(summary.complete);
        assert_eq!(summary.kind, Kind::Past);
        assert!(submit_due(&summary));
        assert_eq!(state_of(&summary).action, Some(Action::Submit));
    }

    #[test]
    fn future_period_is_not_editable_here() {
        let summary = Summary::new(data(date(2026, 11, 1)));
        assert_eq!(summary.kind, Kind::Future);
        assert_eq!(state_of(&summary).label, "futuro");
    }
}
