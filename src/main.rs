use std::io;
use std::error::Error;
use std::io::{Write, ErrorKind};

use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Class, Name, Text};

use termion::input::TermRead;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let client = reqwest::Client::builder()
		.cookie_store(true)
		.build().expect("couldn't create client to connect to internet");

	let (username, password) = get_login_credentials()?;

	let login_page = client.get("https://handins.ccs.neu.edu/login/")
		.send().await?.text().await?;

	let tree = Document::from(login_page.as_str());

	let token = tree.find(Attr("name", "csrf-token"))
		.next().unwrap()
		.attr("content").unwrap();

	let params = [
		("utf8", "%E2%9C%93"), 
		("authenticity_token", token),
		("user[username]", username.as_str()),
		("user[password]", password.as_str()),
		("commit", "Log+in")
	];

	client.post("https://handins.ccs.neu.edu/login/")
		.form(&params)
		.send().await?;

	let assignments = client.get("https://handins.ccs.neu.edu/courses/126/assignments/")
		.header("Referer", "https://handins.ccs.neu.edu/")
		.send().await?
		.text().await?;

	let doc = Document::from(assignments.as_str());

	let assignment_names: Vec<_> = doc
		.find(Name("tbody"))
		.next().unwrap()
		.find(Name("tr"))
		.map(|node| node.find(Name("td")).into_selection().first().unwrap()
			.first_child().unwrap()
			.first_child().unwrap().as_text().unwrap())
		.collect();

	let grades: Vec<Vec<f64>> = doc
		.find(Name("tbody"))
		.next().unwrap()
		.find(Name("tr"))
		.map(|node| node.find(Class("text-right")).into_selection().children())
		.map(|nodes| nodes.iter().filter(|node| node.is(Text)).collect())
		.map(|nodes: Vec<Node>| nodes.iter().map(|node| node.as_text().unwrap().split_whitespace().next().unwrap()).collect())
		.map(|vals: Vec<&str>| vals.iter().map(|val| val.parse::<f64>().unwrap()).collect())
		.collect();

	let _ungraded = grades.iter().filter(|vals| vals.len() == 1);

	let graded: Vec<Vec<f64>> = grades.into_iter().filter(|vals| vals.len() > 1).collect();

	let weights: Vec<f64> = graded.iter().map(|vals| vals[0]).collect();
	let grades: Vec<f64> = graded.iter().map(|vals| vals[1]).collect();

	let width = assignment_names.iter().map(|s| s.len()).max().unwrap();

	let assignment_names = &assignment_names[assignment_names.len() - grades.len()..];

	println!("{:<width$} {:<8} {:>8}", "Homework", "Grades", "Weight", width = width + 5);
	for (name, (grade, weight)) in assignment_names.iter().zip(grades.iter().zip(weights.iter())) {
		let mut fmt_name = String::from(*name);
		fmt_name.push_str(":");

		println!("{:<width$} {:<8.2} {:>8.2}", fmt_name, grade, weight, width = width + 5);
	}
	println!("{:<width$} {:<.2}", "Your current grade:", calculate_grade(grades, weights), width = width + 5);

	Ok(())
}

fn get_login_credentials() -> Result<(String, String), io::Error> {
	loop {
		let stdin = io::stdin();
		let mut stdin = stdin.lock();
		let stdout = io::stdout();
		let mut stdout = stdout.lock();

		print!("username: ");
		stdout.flush().unwrap();

		let username = match stdin.read_line().expect("error reading username") {
			Some(s) if s == "" => {
				return Err(io::Error::new(ErrorKind::InvalidInput, "no username provided"));
			},
			Some(s) => s,
			None => {
				return Err(io::Error::new(ErrorKind::InvalidInput, "no username provided"));
			},
		};

		print!("password: ");
		stdout.flush().unwrap();

		let password = match stdin.read_passwd(&mut stdout).expect("error reading passworld") {
			Some(s) if s == "" => {
				print!("\n");
				return Err(io::Error::new(ErrorKind::InvalidInput, "no password provided"));
			},
			Some(s) => s,
			None => {
				print!("\n");
				return Err(io::Error::new(ErrorKind::InvalidInput, "no password provided"));
			},
		};

		print!("\n");

		break Ok((username, password));
	}
}

fn calculate_grade(grades: Vec<f64>, weights: Vec<f64>) -> f64 {
	assert!(grades.len() == weights.len());

	let total_weight: f64 = weights.iter().sum();

	let scaled_grade = grades.iter().zip(weights.iter())
		.fold(0.0, |sum, grade_pair| grade_pair.0 * grade_pair.1 + sum);

	return scaled_grade / total_weight;
}
