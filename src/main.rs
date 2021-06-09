use std::error::Error;
use std::io;
use std::io::{ErrorKind, Write};
use std::sync::Arc;


use clap::{App, Arg, ArgMatches, SubCommand};

use chrono::DateTime;

use reqwest::{Client, Url};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::multipart::{Form, Part};

use select::document::Document;
use select::predicate::{Attr, Class, Name, Text};

use simsearch::SimSearch;

use tokio::io::AsyncReadExt;

mod assignment;

use crate::assignment::Assignment;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("handins")
		.version("0.1")
		.author("Sam Lyon <sam.c.lyon@gmail.com>")
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
			.arg(Arg::with_name("notes")
				.short("n")
				.long("notes")
				.help("any additional student notes you want to leave for handins")
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

    // using to debug cookie values, if necessary 
    let client = handins_login::<Jar>(None).await?;

    match matches.subcommand() {
        ("grade", Some(sub_matches)) => fetch_grades(&client, sub_matches).await,
        ("submit", Some(sub_matches)) => submit_file(&client, sub_matches).await,
        ("ungraded", Some(sub_matches)) => fetch_ungraded(&client, sub_matches).await,
        _ => Err("must use a supported subcommand with the handins app!")?,
    }
}

async fn fetch_grades(
    client: &Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let course: &str = matches
        .value_of("COURSE")
        .ok_or("you must input a course! supported courses: cs2510, cs2510a")?;

    let course_id = lookup_course(course)
        .map_err(|_| "not a supported course for handins at this time")?;

    let assignments = assignments(client, course_id).await?;

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
    client: &Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let course: &str = matches
        .value_of("COURSE")
        .ok_or("you must input a course! supported courses: cs2510, cs2510a")?;

    let course_id = lookup_course(course)
        .map_err(|_| "not a supported course for handins at this time")?;

    let assignments: Vec<Assignment> = assignments(&client, course_id).await?;
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

async fn submit_file<'b>(
    client: &Client,
    matches: &ArgMatches<'_>,
) -> Result<(), Box<dyn Error>> {
    let file_name: String = matches
        .value_of("FILE")
        .or(matches.value_of("infile"))
        .ok_or("you must input a homework file to submit!")?
        .to_owned();

    let mut file = tokio::fs::File::open(&file_name).await?;
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).await?;

    let course = matches
        .value_of("COURSE")
        .or(matches.value_of("course"))
        .ok_or("you must input a course! use --help to see supported courses")?;

    let course_id = lookup_course(course)
        .map_err(|_| "not a supported course for handins at this time")?;

    let assignment = remove_whitespace(
        matches
            .value_of("ASSIGNMENT")
            .or(matches.value_of("assignment"))
            .ok_or("you must input an assignment to submit your file to!")?,
    );

    let hours = matches.value_of("hours")
        .ok_or("you must input a number of hours you worked on this assignment!")?
        .parse::<f64>()?;
    let notes = matches.value_of("notes")
        .unwrap_or("").to_owned();

    if hours < 0.0 {
        return Err("cannot work on an assignment for negative hours!")?;
    }

    let mut assignments: Vec<Assignment> = assignments(&client, course_id)
        .await?
        .into_iter()
        .filter(|assignment| !assignment.graded())
        .collect();

    // this block of code revolves around getting the correct assignment to submit

    if assignments.is_empty() {
        return Err("all assignments have been graded!")?;
    }
    // sort by reverse date order (most recent first)
    assignments.sort_by(|a1, a2| a2.due_date.cmp(&a1.due_date));

    let submission_candidate_indices = if matches.is_present("recent") {
        vec![0]
    } else {
        let mut engine: SimSearch<usize> = SimSearch::new();
        for (i, item) in assignments.iter().enumerate() {
            engine.insert(i, &item.name);
        }
        engine.search(&assignment)
    };

    // at this point, we need to decide how to parse the assignment submitted by the user.
    // they can either submit an exact (no whitespace) match, or an inexact match. maybe try
    // to implement "A-P" form (A assignment number, P problem number) or "A" form, but this really depends
    // on the class...
    let to_submit = {
        if submission_candidate_indices.is_empty() {
            Err("assignment name didn't match any assignments!")
        } else if matches.is_present("recent") {
            Ok(&assignments[0])
        } else {
            let mut to_submit = Err("couldn't find the right assignment, shutting down");
            for idx in submission_candidate_indices {
                match validate_assignment(&assignments[idx]) {
                    Ok(Some(_)) => {
                        to_submit = Ok(&assignments[idx]);
                        break;
                    }
                    Ok(None) => continue,
                    Err(_) => return Err("error reading from stdin")?,
                }
            }
            to_submit
        }
    }?;

    // we also must check if the assignment would be late, and warn the user if they're trying to submit a late assignment.
    // it's impossible to try to submit to a graded assignment.
    if to_submit.late() {
        print!(
            "{}",
            format!(
                "this assignment is {} hours late! submit anyways? [y/N] ",
                to_submit.how_late()
            )
        );
        io::stdout().flush().unwrap();

        // determine if the user still wants to submit
        loop {
            let mut ans = String::new();
            io::stdin().read_line(&mut ans)?;

            match ans.trim().to_lowercase().as_str() {
                "y" | "yes" => break,
                "n" | "" | "no" => return Err("not submitting assignment, shutting down")?,
                _ => {
                    println!("couldn't get response, trying again...");
                    continue;
                }
            }
        }
    }
    println!("{:?}", to_submit);
    println!("{:?}", to_submit.submission_link(course_id));

    // now, finally, we can construct the request and submit the assignment.
    let submission_page = client
        .get(to_submit.submission_link(course_id))
        .header("Referer", "https://handins.ccs.neu.edu")
        .send()
        .await?
        .text()
        .await?;

    // need to ensure, here, that we land on the correct page: search for distinct element?
    let tree = Document::from(submission_page.as_str());

    let token = tree
        .find(Attr("name", "csrf-token"))
        .next()
        .unwrap()
        .attr("content")
        .unwrap();

    let user_id = tree
        .find(Attr("name", "submission[user_id]"))
        .next()
        .unwrap()
        .attr("value")
        .unwrap();

    println!("{:?}", String::from_utf8(buffer.clone()));

    let file = Part::bytes(buffer)
        .file_name(file_name.clone())
        .mime_str("application/octet-stream")?;

    let submission = Form::new()
        .text("utf8", "✓")
        .text("authenticity_token", token.to_owned())
        .text("submission[type]", "FilesSub")
        .text("submission[assignment_id]", to_submit.id.to_string())
        .text("submission[user_id]", user_id.to_owned())
        .text("submission[time_taken]", format!("{:.1}", hours))
        .text("submission[student_notes]", notes)
        .part("submission[upload_file]", file)
        .text("commit", "Submit files");

    println!("{:?}", submission);

    // DANGER: DO NOT ATTEMPT UNTIL UNGRADED HW AVAILABLE
    // let results_page = client
    //     .post(to_submit.submission_link(course_id))
    //     .multipart(submission)
    //     .header("Referer", to_submit.submission_link(course_id))
    //     .send()
    //     .await?;

    // println!("{:?}", results_page.headers());

    Ok(())
}

async fn handins_login<C: CookieStore + 'static>(store: Option<Arc<C>>) -> Result<Client, Box<dyn Error>> {
    // initialize a new client and login to the user's homepage, so we can do more from there
    let client = {
        if let Some(store) = store {
            Client::builder()
                .cookie_provider(store)
                .build()
                .expect("couldn't create client to connect to internet")
        }
        else {
            Client::builder()
                .cookie_store(true)
                .build()
                .expect("couldn't create client to connect to internet")
        }
    };
     

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
    client: &Client,
    course: i64,
) -> Result<Vec<Assignment>, Box<dyn Error>> {
    let assignments = client
        .get(format!(
            "https://handins.ccs.neu.edu/courses/{}/assignments/",
            course
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
                .rsplit('/')
                .next()
                .unwrap()
                .parse::<i64>()
                .unwrap();
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

fn validate_assignment(assignment: &Assignment) -> Result<Option<&Assignment>, Box<dyn Error>> {
    loop {
        print!(
            "Did you want to submit to the following assignment: {}? [Y/n] ",
            assignment.name
        );
        io::stdout().flush().unwrap();

        let mut ans = String::new();
        io::stdin().read_line(&mut ans)?;
        let ans = ans.trim();
        if ans.is_empty() || ans.to_lowercase() == "y" {
            return Ok(Some(assignment));
        } else if ans.to_lowercase() == "n" {
            return Ok(None);
        } else {
            println!("Couldn't get response, trying again...");
        }
    }
}

// spring 2021 courses
// will probably add a macro to convert a file w course names, number
// to a lookup table, if numbers get updated each year
fn lookup_course(course: &str) -> Result<i64, &str> {
    match course.to_lowercase().as_str() {
        "cs2500" | "fundies1" | "f1" => Ok(131),
        "cs2510" | "fundies2" | "f2" => Ok(129),
        "cs2510a" | "fundies2accel" | "f2accel" | "f2a" => Ok(126),
        "cs3500" | "ood" => Ok(138),
        "cs3500sp21" | "oodsp21" => Ok(133),
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
