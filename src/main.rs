use std::error::Error;
use std::fs;
use std::io;
use std::io::{ErrorKind, Write};

use clap::{App, Arg, ArgMatches, SubCommand};

use chrono::{DateTime, FixedOffset, Local};

use select::document::Document;
use select::predicate::{Attr, Class, Name, Text};

// represents an assignment with additional metadata from scraping: the
// name, relative link, if the assignment was graded, and its due date
#[derive(Debug)]
struct Assignment {
    name: String,
    link: String,
    grade: Option<f64>,
    weight: f64,
    due_date: DateTime<FixedOffset>,
}

impl Assignment {
    pub fn new(
        name: String,
        link: String,
        grade: Option<f64>,
        weight: f64,
        due_date: DateTime<FixedOffset>,
    ) -> Assignment {
        Assignment {
            name,
            link,
            grade,
            weight,
            due_date,
        }
    }

    pub fn late(&self) -> bool {
        let now = Local::now();
        now >= self.due_date
    }

    pub fn graded(&self) -> bool {
        self.grade.is_some()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("handins")
		.version("0.1")
		.author("Sam Lyon <sam.c.lyon@gmail.com")
		.about("Command line interface for handins.ccs.neu.edu")
		.subcommand(SubCommand::with_name("grade")
			.about("fetches your grades for a given course")
			.version("0.1")
			.author("Sam Lyon <sam.c.lyon@gmail.com")
			.arg(Arg::with_name("COURSE")
				.help("name of the course taken (cs2510, cs2510a)")
				.long_help(
"Name of the course to retrieve grades from. Supports the following courses:\n\
cs2510   	--		Fundamentals of Computer Science 2\n\
cs2510a  	--		Fundamentals of Computer Science 2 Accelerated\n"
				)
				.required(true)
				.index(1))
		)
		.subcommand(SubCommand::with_name("ungraded")
			.about("fetches your ungraded assignments for a given course")
			.version("0.1")
			.author("Sam Lyon <sam.c.lyon@gmail.com")
			.arg(Arg::with_name("COURSE")
				.help("name of the course taken (cs2510, cs2510a)")
				.long_help(
"Name of the course to retrieve grades from. Supports the following courses:\n\
cs2510   	--		Fundamentals of Computer Science 2\n\
cs2510a  	--		Fundamentals of Computer Science 2 Accelerated\n"
				)
				.required(true)
				.index(1))
		)
		.subcommand(SubCommand::with_name("submit")
			.about("submits your file to the class/assignment specified: not implemented yet")
			.version("0.1")
			.author("Sam Lyon <sam.c.lyon@gmail.com")
			.arg(Arg::with_name("FILE")
				.help("path to the file to submit")
				.required_unless("file")
				.index(1))
			.arg(Arg::with_name("COURSE")
				.help("class to submit your file to")
				.required_unless("course")
				.index(2))
			.arg(Arg::with_name("ASSIGNMENT")
				.help("name of the assignment to submit to")
				.required_unless("name")
				.required_unless("recent")
				.index(3))
			.arg(Arg::with_name("file")
				.short("i")
				.long("infile")
				.help("input file to submit to handins")
				.takes_value(true))
			.arg(Arg::with_name("course")
				.short("c")
				.long("course")
				.help("class to submit your file to")
				.takes_value(true))
			.arg(Arg::with_name("name")
				.short("a")
				.long("assignment")
				.help("name of the assignment to submit to")
				.takes_value(true))
			.arg(Arg::with_name("hours")
				.short("H")
				.long("hours")
				.help("number of hours you worked on the homework submitted")
				.required(true)
				.takes_value(true))
			.arg(Arg::with_name("comments")
				.short("C")
				.long("comments")
				.help("any additional comments you want to leave for handins")
				.takes_value(true))
			.arg(Arg::with_name("wait")
				.short("w")
				.long("wait")
				.help("wait for the grade feedback to generate, and print it out afterwards"))
			.arg(Arg::with_name("recent")
				.short("r")
				.long("recent")
				.help("choose the most recently assigned homework to submit to")))
	.get_matches();

    match matches.subcommand() {
        ("grade", Some(sub_matches)) => fetch_grades(&handins_login().await?, &sub_matches).await,
        ("submit", Some(sub_matches)) => submit_file(&handins_login().await?, &sub_matches).await,
        ("ungraded", Some(sub_matches)) => {
            fetch_ungraded(&handins_login().await?, &sub_matches).await
        }
        _ => Err("must use a supported subcommand with the handins app!")?,
    }
}

async fn fetch_grades(
    client: &reqwest::Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let course: &str = matches
        .value_of("COURSE")
        .ok_or("you must input a course! supported courses: cs2510, cs2510a")?;

    let assignments = assignments(client, course).await?;

    let width = assignments.iter().map(|s| s.name.len()).max().unwrap();

    let (cur_grade, min_grade, max_grade, max_points) = calculate_grade(&assignments);

    println!(
        "{:<width$} {:<8} {:>8}",
        "Homework",
        "Grades",
        "Weight",
        width = width + 5
    );

    for assignment in assignments {
        let fmt_name = format!("{}:", assignment.name);

        if let Some(grade) = assignment.grade {
            println!(
                "{:<width$} {:<8.2} {:>8.2}",
                fmt_name,
                grade,
                assignment.weight,
                width = width + 5
            );
        }
    }
    println!(
        "{:<width$} {:<.2}",
        "Your current grade:",
        cur_grade,
        width = width + 5
    );
    println!(
        "{:<width$} {:<.2}",
        "Your minimum grade:",
        min_grade,
        width = width + 5
    );
    println!(
        "{:<width$} {:<.2}",
        "Your maximum grade:",
        max_grade,
        width = width + 5
    );
    println!(
        "{:<width$} {:<.2}",
        "Ungraded points you can earn:",
        max_points,
        width = width + 5
    );

    Ok(())
}

async fn fetch_ungraded(
    client: &reqwest::Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let course: &str = matches
        .value_of("COURSE")
        .ok_or("you must input a course! supported courses: cs2510, cs2510a")?;

    let assignments: Vec<Assignment> = assignments(&client, course).await?;
    let ungraded_assignments: Vec<&Assignment> =
        assignments.iter().filter(|a| a.grade.is_none()).collect();
    let width = ungraded_assignments
        .iter()
        .map(|a| a.name.len())
        .max()
        .unwrap();

    println!(
        "{:<width$} {:<8}",
        "Assignment",
        "Weight",
        width = width + 5
    );

    for assignment in ungraded_assignments {
        println!(
            "{:<width$} {:<.2}",
            assignment.name,
            assignment.weight,
            width = width + 5
        );
    }

    Ok(())
}

async fn submit_file(
    client: &reqwest::Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let file = matches
        .value_of("FILE")
        .or(matches.value_of("infile"))
        .ok_or("you must input a homework file to submit!")?;

    let file = fs::canonicalize(file).map_err(|_| "you must input a valid file name!")?;

    let course = matches
        .value_of("COURSE")
        .or(matches.value_of("course"))
        .ok_or("you must input a course! use --help to see supported courses")?;

    let assignment = remove_whitespace(
        matches
            .value_of("ASSIGNMENT")
            .or(matches.value_of("assignment"))
            .ok_or("you must input an assignment to submit your file to!")?,
    );

    let assignments = assignments(&client, course);

    unimplemented!();
    // at this point, we need to decide how to parse the assignment submitted by the user.
    // they can either submit an exact (no whitespace) match, or an inexact match. maybe try
    // to implement "A-P" form (A assignment number, P problem number) or "A" form, but this really depends
    // on the class...

    // we also must check if the assignment would be late, and warn the user if they're trying to submit a late assignment.
    // if the assignment is already graded, we shouldn't let them submit. no way to know late days, but this would also
    // cause an error
}

async fn handins_login() -> Result<reqwest::Client, Box<dyn Error>> {
    // initialize a new client and login to the user's homepage, so we can do more from there
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("couldn't create client to connect to internet");

    let (username, password) = get_login_credentials()?;

    let login_page = client
        .get("https://handins.ccs.neu.edu/login/")
        .send()
        .await?
        .text()
        .await?;

    let tree = Document::from(login_page.as_str());

    // we need to grab the csrf-token from the metadata in the head, since
    // it's not properly set in the form initially
    let token = tree
        .find(Attr("name", "csrf-token"))
        .next()
        .unwrap()
        .attr("content")
        .unwrap();

    let params = [
        ("utf8", "%E2%9C%93"),
        ("authenticity_token", token),
        ("user[username]", username.as_str()),
        ("user[password]", password.as_str()),
        ("commit", "Log+in"),
    ];

    client
        .post("https://handins.ccs.neu.edu/login/")
        .form(&params)
        .send()
        .await?;

    // client is now logged in with the user session, so return it
    Ok(client)
}

async fn assignments(
    client: &reqwest::Client,
    course: &str,
) -> Result<Vec<Assignment>, Box<dyn Error>> {
    let assignments = client
        .get(format!(
            "https://handins.ccs.neu.edu/courses/{}/assignments/",
            lookup_course(course)?
        ))
        .header("Referer", "https://handins.ccs.neu.edu/")
        .send()
        .await?
        .text()
        .await?;

    let tree = Document::from(assignments.as_str());

    let assignments: Vec<Assignment> = tree
        .find(Name("tbody"))
        .next()
        .unwrap()
        .find(Name("tr"))
        .map(|row| {
            let row_selection = row.find(Name("td")).into_selection();
            let link = row_selection
                .find(Attr("href", ()))
                .first()
                .unwrap()
                .attr("href")
                .unwrap()
                .to_owned();
            let name = row_selection.find(Text).first().unwrap().text();
            let date = DateTime::parse_from_rfc3339(
                &row_selection
                    .find(Class("local-time"))
                    .first()
                    .unwrap()
                    .text(),
            )
            .unwrap();
            let weight = row
                .find(Class("text-right"))
                .next()
                .unwrap()
                .text()
                .trim()
                .to_owned();
            let grade = row
                .find(Class("text-right"))
                .into_selection()
                .next()
                .next()
                .first()
                .unwrap()
                .first_child()
                .filter(|grade| grade.is(Text))
                .map(|grade| grade.text());

            let weight = weight.parse::<f64>().unwrap();
            let grade = grade.map(|grade| grade.parse::<f64>().ok()).flatten();


            Assignment::new(name, link, grade, weight, date)
        })
        .collect();

    Ok(assignments)
}

fn get_login_credentials() -> Result<(String, String), io::Error> {
    print!("username: ");
    io::stdout().flush().unwrap();

    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    if username.is_empty() {
        println!();
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "no username provided!",
        ));
    }

    let password = match rpassword::read_password_from_tty(Some("password: ")) {
        Ok(s) if s.is_empty() => {
            println!();
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "no password provided",
            ));
        }
        Ok(s) => s,
        Err(e) => {
            println!();
            return Err(e);
        }
    };

    Ok((username, password))
}

// spring 2021 courses
// will probably add a macro to convert a file w course names, number
// to a lookup table, if numbers get updated each year
fn lookup_course(course: &str) -> Result<u32, &str> {
    match course.to_lowercase().as_str() {
        "cs2500" | "fundies1" | "f1" => Ok(131),
        "cs2510" | "fundies2" | "f2" => Ok(129),
        "cs2510a" | "fundies2accel" | "f2accel" | "f2a" => Ok(126),
        "cs3500" | "ood" => Ok(133),
        "cs4410" | "compilers" => Ok(127),
        "cs4500" | "swdev" | "swe" => Ok(130),
        _ => Err("Course not found"),
    }
}

fn calculate_grade(assignments: &[Assignment]) -> (f64, f64, f64, f64) {
    let valid_weights: Vec<f64> = assignments
        .iter()
        .filter_map(|a| a.grade.map(|_| a.weight))
        .collect();

    let future_weights: Vec<f64> = assignments
        .iter()
        .filter_map(|a| match a.grade {
            Some(_) => None,
            None => Some(a.weight),
        })
        .collect();

    let total_weight: f64 = valid_weights.iter().sum();

    let grades: Vec<f64> = assignments.iter().filter_map(|a| a.grade).collect();

    let scaled_grade = grades
        .iter()
        .zip(valid_weights.iter())
        .fold(0.0, |sum, grade_pair| grade_pair.0 * grade_pair.1 + sum);

    let future_weight: f64 = future_weights.iter().sum();
    let optimistic_grade = scaled_grade + 100.0 * (100.0 - total_weight);

    (
        scaled_grade / total_weight, // your current grade
        scaled_grade / 100.0,        // your minimum grade
        optimistic_grade / 100.0,    // maximum possible grade for the course
        // most points you can earn from ungraded assignments
        // delta (max possible grade from ungraded assignments, current grade)
        (scaled_grade + 100.0 * future_weight) / (total_weight + future_weight)
            - (scaled_grade / total_weight),
    )
}

fn remove_whitespace(s: &str) -> String {
    s.replace(char::is_whitespace, "")
}
