use std::collections::BTreeMap;
use std::io::{self, IsTerminal};
use std::thread;

use chrono::Datelike;
use serde_json::Value;

use crate::api::{Api, Period};
use crate::browser;
use crate::config::{self, Config};
use crate::dates;
use crate::error::{msg, AppError, Result};
use crate::log;
use crate::login;
use crate::model::{self, Action, Data, Kind, PlanOptions, Summary};
use crate::ui;

const FILL_USAGE: &str = "uso: timesheet fill [periodo] [--date DÍA] [--dry-run] [-f]

  sin periodo   elige entre los periodos pendientes
  periodo       current | 2026-11-01 | 01-Nov-2026
  --date DÍA    rellena solo ese día
  --dry-run     muestra lo que haría, sin escribir
  -f            sobrescribe horas ya registradas";

const SUBMIT_USAGE: &str = "uso: timesheet submit [periodo] [--dry-run] [-f]

  sin periodo   elige entre los periodos listos para enviar
  periodo       current | 2026-11-01 | 01-Nov-2026
  --dry-run     valida y muestra, sin enviar
  -f            envía aunque falten días";

const STATUS_USAGE: &str = "uso: timesheet [status] [periodo] [-n N]

  periodo   current | 2026-11-01 | 01-Nov-2026
  -n N      periodos anteriores a revisar (por defecto 3)";

const LIST_USAGE: &str = "uso: timesheet list [-n N] [--all]

  -n N     periodos anteriores a mostrar (por defecto 6)
  --all    incluye los ya enviados o aprobados";

const INIT_USAGE: &str = "uso: timesheet init [--emp CODE] [--project CODE] [--hours N]

  --emp       código de empleado en iEngage
  --project   código de proyecto (ej. 20788-7)
  --hours     horas por día laborable (por defecto 8)";

const LOGIN_USAGE: &str = "uso: timesheet login [--timeout N] [--browser RUTA]

  abre iEngage en tu navegador y guarda la sesión
  --timeout N     minutos de espera (por defecto 10)
  --browser RUTA  usa un navegador concreto

avanzado:
  --cookie [ARCHIVO]  importa una cookie a mano (headless/CI)";

#[derive(Default)]
struct WriteOptions {
    period: Option<String>,
    dry_run: bool,
    force: bool,
    date: Option<String>,
}

#[derive(Default)]
struct Outcome {
    submitted: usize,
}

enum Selection {
    All,
    One(usize),
}

pub fn status(args: &[String]) -> Result<()> {
    list_common(args, 3, STATUS_USAGE)
}

pub fn list(args: &[String]) -> Result<()> {
    list_common(args, 6, LIST_USAGE)
}

fn list_common(args: &[String], default_back: usize, usage: &str) -> Result<()> {
    let mut back = default_back;
    let mut all = false;
    let mut positionals = Vec::new();
    let mut iter = args.iter().cloned();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-n" => back = parse_count(&take_value(&mut iter, "-n")?)?,
            "--all" | "-a" => all = true,
            "-h" | "--help" => {
                println!("{usage}");
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(msg(format!("opción desconocida: {other} · {usage}")))
            }
            other => positionals.push(other.to_string()),
        }
    }

    let config = Config::load()?;
    let api = Api::new(&config, read_cookie()?);
    let today = dates::today();
    let target = positionals.first().map(String::as_str);
    let current = Summary::new(data_for(&api, today, target)?);

    if target.is_some() {
        return show(vec![current], all);
    }

    let previous = previous_periods(&api, &current.data.period, today, back)?;
    let mut summaries = vec![current];
    summaries.extend(fetch_many(&api, &previous, today)?);
    show(summaries, all)
}

pub fn run_fill(args: &[String]) -> Result<()> {
    let Some(options) = parse_write(args, FILL_USAGE, true)? else {
        return Ok(());
    };
    let config = Config::load()?;
    let api = Api::new(&config, read_cookie()?);
    let today = dates::today();
    let only_date = options.date.as_deref().map(dates::parse_any).transpose()?;

    if let Some(period) = options.period.as_deref() {
        let data = data_for(&api, today, Some(period))?;
        fill_one(&api, &config, data, &options, only_date)?;
        return Ok(());
    }

    if !io::stdin().is_terminal() {
        return Err(msg("indica el periodo · timesheet fill 2026-11-01"));
    }

    let pending = pending_for(&api, today, Action::Fill)?;
    let Some(selection) = choose(&pending, "¿Qué periodo?", "nada pendiente")? else {
        return Ok(());
    };

    match selection {
        Selection::All => {
            let submitted = apply_all(&pending, |summary| {
                fill_one(&api, &config, summary.data.clone(), &options, only_date)
            })?;
            print_batch(pending.len(), submitted);
        }
        Selection::One(index) => {
            fill_one(
                &api,
                &config,
                pending[index].data.clone(),
                &options,
                only_date,
            )?;
        }
    }
    Ok(())
}

fn fill_one(
    api: &Api,
    config: &Config,
    data: Data,
    options: &WriteOptions,
    only_date: Option<chrono::NaiveDate>,
) -> Result<Outcome> {
    let head = dates::range(data.period.start, data.period.end, data.today);
    let before = Summary::new(data.clone());

    if before.kind == Kind::Future {
        return Err(msg(format!("periodo futuro · {head}")));
    }
    if !before.editable {
        ui::ok(&format!("ya enviado · {head}"));
        return Ok(Outcome::default());
    }

    let plan = model::plan(
        config,
        &data,
        &PlanOptions {
            only_date,
            force: options.force,
            allow_new: true,
            ..Default::default()
        },
    )?;
    let saved = plan.targets.len();

    if plan.targets.is_empty() {
        if model::submit_due(&before) {
            if options.dry_run {
                ui::ok(&format!(
                    "listo para enviar · {head} ({}/{}) · dry-run",
                    before.filled, before.total
                ));
                return Ok(Outcome::default());
            }
            submit_period(api, &data, &plan.entries)?;
            return Ok(Outcome { submitted: 1 });
        }

        ui::ok(&format!(
            "al día · {head} ({}/{})",
            before.filled, before.total
        ));
        print_next(&before, "vuelve mañana");
        return Ok(Outcome::default());
    }

    let new_days = plan
        .targets
        .iter()
        .filter(|day| before.hours_on(&day.api) == 0.0)
        .count();
    let after = before.filled + new_days;
    let will_submit = after == before.total
        && (before.kind == Kind::Past || before.last_workday == Some(data.today));
    let total_hours = saved as f64 * config.hours_per_day;

    if options.dry_run {
        ui::say(&format!(
            "{head}  {}/{} → {}/{}",
            before.filled, before.total, after, before.total
        ));
        for day in &plan.targets {
            ui::say(&format!(
                "{}  {}",
                dates::day_month_weekday(day.date),
                ui::hours(config.hours_per_day)
            ));
        }
        ui::say(&ui::dim(&format!(
            "dry-run · {} · {}{}",
            ui::days_text(saved),
            ui::hours(total_hours),
            if will_submit { " · luego envía" } else { "" }
        )));
        return Ok(Outcome::default());
    }

    api.persist(&plan.entries, "Save")?;
    log::line(&format!(
        "Save OK · {saved} días · {}",
        ui::hours(total_hours)
    ));

    let mut after_data = data.clone();
    after_data.grid = api.grid(&data.period)?;
    let summary = Summary::new(after_data.clone());

    if !plan
        .targets
        .iter()
        .all(|day| summary.hours_on(&day.api) > 0.0)
    {
        return Err(msg("el servidor no confirmó las horas"));
    }

    ui::say(&format!(
        "{head}  {}/{} → {}/{}",
        before.filled, before.total, summary.filled, summary.total
    ));
    ui::say(&format!(
        "{} · {} · {}",
        ui::dim("guardado"),
        ui::days_text(saved),
        ui::hours(total_hours)
    ));

    if summary.complete && (summary.kind == Kind::Past || summary.last_workday == Some(data.today))
    {
        let entries = model::plan(config, &after_data, &PlanOptions::default())?.entries;
        submit_period(api, &after_data, &entries)?;
        return Ok(Outcome { submitted: 1 });
    }

    print_next(&summary, &head);
    Ok(Outcome::default())
}

fn submit_period(api: &Api, data: &Data, entries: &[Value]) -> Result<()> {
    api.persist(entries, "Submit")?;
    log::line(&format!(
        "Submit OK · {} - {}",
        data.period.start_api, data.period.end_api
    ));
    ui::ok(&format!(
        "enviado a aprobación · {}",
        dates::range(data.period.start, data.period.end, data.today)
    ));
    Ok(())
}

fn print_next(summary: &Summary, wait: &str) {
    if summary.complete {
        if let Some(day) = summary.last_workday {
            ui::say(&format!("→ submit el {}", dates::day_month_weekday(day)));
        }
    } else if let Some(day) = summary.missing_future.first() {
        ui::say(&format!(
            "→ {} pendiente",
            dates::day_month_weekday(day.date)
        ));
    } else if !summary.missing_now.is_empty() {
        ui::warn(&format!(
            "quedan {} · {wait}",
            ui::days_text(summary.missing_now.len())
        ));
    }
}

pub fn run_submit(args: &[String]) -> Result<()> {
    let Some(options) = parse_write(args, SUBMIT_USAGE, false)? else {
        return Ok(());
    };
    let config = Config::load()?;
    let api = Api::new(&config, read_cookie()?);
    let today = dates::today();

    if let Some(period) = options.period.as_deref() {
        let data = data_for(&api, today, Some(period))?;
        submit_one(&api, &config, data, &options)?;
        return Ok(());
    }

    if !io::stdin().is_terminal() {
        return Err(msg("indica el periodo · timesheet submit 2026-11-01"));
    }

    let pending = pending_for(&api, today, Action::Submit)?;
    let Some(selection) = choose(&pending, "¿Qué periodo enviar?", "nada que enviar")? else {
        return Ok(());
    };

    match selection {
        Selection::All => {
            let submitted = apply_all(&pending, |summary| {
                submit_one(&api, &config, summary.data.clone(), &options)
            })?;
            print_batch(pending.len(), submitted);
        }
        Selection::One(index) => {
            submit_one(&api, &config, pending[index].data.clone(), &options)?;
        }
    }
    Ok(())
}

fn submit_one(api: &Api, config: &Config, data: Data, options: &WriteOptions) -> Result<Outcome> {
    let head = dates::range(data.period.start, data.period.end, data.today);
    let summary = Summary::new(data.clone());

    if summary.kind == Kind::Future {
        return Err(msg(format!("periodo futuro · {head}")));
    }
    if !summary.editable {
        ui::ok(&format!("ya enviado · {head}"));
        return Ok(Outcome::default());
    }
    if !summary.complete && !options.force {
        return Err(msg(format!(
            "faltan {} · timesheet fill {}",
            ui::days_text(summary.total - summary.filled),
            dates::ymd(data.period.start)
        )));
    }
    if options.dry_run {
        ui::ok(&format!(
            "listo para enviar · {head} ({}/{}) · dry-run",
            summary.filled, summary.total
        ));
        return Ok(Outcome::default());
    }

    let entries = model::plan(config, &data, &PlanOptions::default())?.entries;
    submit_period(api, &data, &entries)?;
    Ok(Outcome { submitted: 1 })
}

pub fn run_init(args: &[String]) -> Result<()> {
    let mut emp = None;
    let mut project = None;
    let mut hours = None;
    let mut iter = args.iter().cloned();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--emp" => emp = Some(take_value(&mut iter, "--emp")?),
            "--project" => project = Some(take_value(&mut iter, "--project")?),
            "--hours" => hours = Some(take_value(&mut iter, "--hours")?),
            "-h" | "--help" => {
                println!("{INIT_USAGE}");
                return Ok(());
            }
            other => return Err(msg(format!("opción desconocida: {other} · {INIT_USAGE}"))),
        }
    }

    let exists = Config::exists();
    let mut config = if exists {
        Config::load()?
    } else {
        Config::default()
    };
    let given = emp.is_some() || project.is_some() || hours.is_some();

    if exists && !given && !io::stdin().is_terminal() {
        ui::ok(&format!(
            "ya configurado · {}",
            pretty_path(&config::config_path()?)
        ));
        return Ok(());
    }

    if let Some(value) = emp {
        config.emp_code = value;
    }
    if let Some(value) = project {
        config.project_code = value;
    }
    if let Some(value) = hours {
        config.hours_per_day = value.parse().map_err(|_| msg("horas por día inválidas"))?;
    }

    if !given {
        config.emp_code = ui::prompt("código de empleado", &config.emp_code)?;
        config.project_code = ui::prompt("código de proyecto", &config.project_code)?;
        let default_hours = format!("{}", config.hours_per_day);
        config.hours_per_day = ui::prompt("horas por día", &default_hours)?
            .parse()
            .map_err(|_| msg("horas por día inválidas"))?;
    }

    config.emp_code = config.emp_code.trim().to_string();
    config.project_code = config.project_code.trim().to_string();

    if config.emp_code.len() < 4
        || config.emp_code.len() > 12
        || !config.emp_code.chars().all(|c| c.is_ascii_digit())
    {
        return Err(msg("código de empleado inválido"));
    }
    if config.project_code.is_empty()
        || !config
            .project_code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(msg("código de proyecto inválido"));
    }
    if !config.hours_per_day.is_finite() || config.hours_per_day <= 0.0 {
        return Err(msg("horas por día inválidas"));
    }

    let path = config.save()?;
    println!();
    ui::ok(&format!("configuración guardada · {}", pretty_path(&path)));
    ui::hint("timesheet login");
    Ok(())
}

pub fn run_login(args: &[String]) -> Result<()> {
    let mut source = None;
    let mut browser = None;
    let mut timeout = 10u64;
    let mut iter = args.iter().cloned().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--cookie" => {
                let path = match iter.peek() {
                    Some(next) if !next.starts_with('-') => iter.next(),
                    _ => None,
                };
                source = Some(match path {
                    Some(path) => login::CookieSource::File(path),
                    None => login::CookieSource::Stdin,
                });
            }
            "--browser" => browser = Some(take_value(&mut iter, "--browser")?),
            "--timeout" => {
                let value = take_value(&mut iter, "--timeout")?;
                timeout = value
                    .parse::<u64>()
                    .ok()
                    .filter(|minutes| *minutes > 0)
                    .ok_or_else(|| msg("--timeout debe ser un entero de minutos > 0"))?;
            }
            "-h" | "--help" => {
                println!("{LOGIN_USAGE}");
                return Ok(());
            }
            other => return Err(msg(format!("opción desconocida: {other} · {LOGIN_USAGE}"))),
        }
    }

    let config = Config::load()?;
    match source {
        Some(source) => login::from_cookie(&config, source),
        None => browser::login(&config, browser.as_deref(), timeout),
    }
}

fn parse_write(args: &[String], usage: &str, allow_date: bool) -> Result<Option<WriteOptions>> {
    let mut options = WriteOptions::default();
    let mut iter = args.iter().cloned();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dry-run" => options.dry_run = true,
            "-f" | "--force" => options.force = true,
            "--date" if allow_date => options.date = Some(take_value(&mut iter, "--date")?),
            "-h" | "--help" => {
                println!("{usage}");
                return Ok(None);
            }
            other if other.starts_with('-') => {
                return Err(msg(format!("opción desconocida: {other} · {usage}")))
            }
            other => {
                if options.period.is_some() {
                    return Err(msg(format!("argumento de más: {other} · {usage}")));
                }
                options.period = Some(other.to_string());
            }
        }
    }

    Ok(Some(options))
}

fn data_for(api: &Api, today: chrono::NaiveDate, value: Option<&str>) -> Result<Data> {
    let period = match value {
        Some(value) if is_current(value) => api.current_period(today)?,
        Some(value) => api.current_period(dates::parse_any(value)?)?,
        None => api.current_period(today)?,
    };
    data_for_period(api, period, today)
}

fn data_for_period(api: &Api, period: Period, today: chrono::NaiveDate) -> Result<Data> {
    let emp = api.employee(&period)?;
    let days = api.days(&period)?;
    let grid = api.grid(&period)?;
    Ok(Data {
        period,
        emp,
        days,
        grid,
        today,
    })
}

fn is_current(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "current" | "now" | "today" | "." | "-0"
    )
}

fn previous_periods(
    api: &Api,
    current: &Period,
    today: chrono::NaiveDate,
    count: usize,
) -> Result<Vec<Period>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let year = today.year();
    let mut catalog = BTreeMap::new();
    for year in [year - 1, year] {
        for period in api.periods(year)? {
            catalog.entry(period.start).or_insert(period);
        }
    }
    let all: Vec<Period> = catalog.into_values().collect();
    let Some(index) = all
        .iter()
        .position(|period| period.start_api == current.start_api)
    else {
        return Ok(Vec::new());
    };

    Ok(all[index.saturating_sub(count)..index]
        .iter()
        .rev()
        .cloned()
        .collect())
}

fn fetch_many(api: &Api, periods: &[Period], today: chrono::NaiveDate) -> Result<Vec<Summary>> {
    if periods.is_empty() {
        return Ok(Vec::new());
    }
    ui::progress(&format!("consultando {} periodos…", periods.len()));

    let results: Vec<Result<Summary>> = thread::scope(|scope| {
        let handles: Vec<_> = periods
            .iter()
            .map(|period| {
                let api = api.clone();
                let period = period.clone();
                scope.spawn(move || {
                    let data = data_for_period(&api, period, today)?;
                    Ok(Summary::new(data))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(msg("consulta interrumpida")))
            })
            .collect()
    });

    results.into_iter().collect()
}

fn pending_for(api: &Api, today: chrono::NaiveDate, action: Action) -> Result<Vec<Summary>> {
    let current = Summary::new(data_for(api, today, None)?);
    let previous = previous_periods(api, &current.data.period, today, 6)?;
    let mut summaries = vec![current];
    summaries.extend(fetch_many(api, &previous, today)?);
    Ok(summaries
        .into_iter()
        .filter(|summary| model::state_of(summary).action == Some(action))
        .collect())
}

fn choose(pending: &[Summary], title: &str, empty: &str) -> Result<Option<Selection>> {
    if pending.is_empty() {
        ui::ok(empty);
        return Ok(None);
    }

    let all = pending.len() > 1;
    let mut items = Vec::new();
    if all {
        items.push(format!("todos los pendientes ({})", pending.len()));
    }
    items.extend(pending.iter().map(item_label));

    let Some(choice) = ui::select(title, &items)? else {
        return Ok(None);
    };
    Ok(Some(if all && choice == 0 {
        Selection::All
    } else {
        Selection::One(if all { choice - 1 } else { choice })
    }))
}

fn apply_all(
    pending: &[Summary],
    mut apply: impl FnMut(&Summary) -> Result<Outcome>,
) -> Result<usize> {
    let mut submitted = 0;
    for (index, summary) in pending.iter().rev().enumerate() {
        if index > 0 {
            println!();
        }
        submitted += apply(summary)?.submitted;
    }
    Ok(submitted)
}

fn print_batch(periods: usize, submitted: usize) {
    println!();
    ui::ok(&format!(
        "{} · {}",
        ui::plural(periods, "periodo", "periodos"),
        ui::plural(submitted, "enviado", "enviados")
    ));
}

fn state_label(summary: &Summary) -> String {
    let state = model::state_of(summary);
    match state.color {
        Some(code) => ui::paint(code, state.label),
        None => state.label.to_string(),
    }
}

fn show(summaries: Vec<Summary>, all: bool) -> Result<()> {
    let visible: Vec<&Summary> = summaries
        .iter()
        .enumerate()
        .filter(|(index, summary)| all || *index == 0 || model::state_of(summary).action.is_some())
        .map(|(_, summary)| summary)
        .collect();

    if ui::verbose() {
        for summary in &visible {
            print_detail(summary);
        }
    }

    let rows: Vec<Vec<String>> = visible
        .iter()
        .map(|summary| {
            vec![
                dates::range(
                    summary.data.period.start,
                    summary.data.period.end,
                    summary.data.today,
                ),
                format!("{}/{}", summary.filled, summary.total),
                state_label(summary),
            ]
        })
        .collect();

    ui::table(&["periodo", "días", "estado"], &rows, &[]);
    print_actions(&visible);
    Ok(())
}

fn print_actions(summaries: &[&Summary]) {
    let mut commands: Vec<&str> = Vec::new();
    for summary in summaries {
        if let Some(action) = model::state_of(summary).action {
            let command = action.command();
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
    }
    if commands.is_empty() {
        return;
    }
    println!();
    for command in commands.iter().take(3) {
        ui::hint(command);
    }
}

fn print_detail(summary: &Summary) {
    let emp = &summary.data.emp;
    println!(
        "{}  {} · {}",
        ui::dim("empleado"),
        field(emp, "EmpCode"),
        field(emp, "EmpName")
    );
    println!("{}    {}", ui::dim("estado"), field(emp, "StatusDesc"));
    println!(
        "{}   {}",
        ui::dim("periodo"),
        dates::range(
            summary.data.period.start,
            summary.data.period.end,
            summary.data.today
        )
    );
    if summary.is_new {
        println!("{}     periodo sin inicializar (TID=0)", ui::dim("aviso"));
    }
    println!();

    let rows: Vec<Vec<String>> = summary
        .info
        .iter()
        .map(|day| {
            let hours = summary.hours_on(&day.api);
            vec![
                dates::day_month_weekday(day.date),
                day.day_type.clone(),
                if hours > 0.0 {
                    ui::hours(hours)
                } else {
                    "-".to_string()
                },
                day_state(day, summary, hours),
            ]
        })
        .collect();
    ui::table(&["fecha", "día", "horas", "estado"], &rows, &[2]);
    println!();
}

fn day_state(day: &model::Day, summary: &Summary, hours: f64) -> String {
    if day.day_id == 2 {
        return "finde".to_string();
    }
    if day.day_id == 4 {
        return "feriado".to_string();
    }
    if day.date > summary.data.today {
        return "futuro".to_string();
    }
    if hours > 0.0 {
        "ok".to_string()
    } else {
        "falta".to_string()
    }
}

fn item_label(summary: &Summary) -> String {
    format!(
        "{}  {}/{}  {}",
        dates::range(
            summary.data.period.start,
            summary.data.period.end,
            summary.data.today
        ),
        summary.filled,
        summary.total,
        state_label(summary)
    )
}

fn read_cookie() -> Result<String> {
    let path = config::cookie_path()?;
    let raw = std::fs::read_to_string(&path).map_err(|_| {
        AppError::Auth(format!(
            "no existe el archivo de cookie: {}",
            path.display()
        ))
    })?;
    crate::api::parse_cookie(&raw)
        .ok_or_else(|| AppError::Auth("el archivo de cookie está vacío".into()))
}

fn take_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    iter.next()
        .ok_or_else(|| msg(format!("{flag} necesita un valor")))
}

fn parse_count(value: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| msg("--n debe ser un entero >= 0"))
}

fn pretty_path(path: &std::path::Path) -> String {
    match config::home() {
        Ok(home) => path
            .to_string_lossy()
            .replacen(&home.to_string_lossy().to_string(), "~", 1),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

fn field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}
