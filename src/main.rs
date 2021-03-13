use std::env;
use std::io;
use std::error::Error;
use std::io::{Write, ErrorKind};

use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Class, Name, Text};


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let mut args = env::args();
	args.next();

	let course: String = args.collect();
	if course.is_empty() {
		return Err("you must input a course! supported courses: cs2510, cs2510a")?;
	}

	let course_id = lookup_course(&course).unwrap_or(129);

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

	let course_url = format!("https://handins.ccs.neu.edu/courses/{}/assignments/", course_id);

	let (assignment_names, grades, weights, ungraded) = 
		scrape_grades(&client, &course_url).await?;

	let width = assignment_names.iter().map(|s| s.len()).max().unwrap();

	let assignment_names = &assignment_names[assignment_names.len() - grades.len()..];

	let (cur_grade, min_grade, max_grade, max_points) = calculate_grade(&grades, &weights, &ungraded);

	println!("{:<width$} {:<8} {:>8}", "Homework", "Grades", "Weight", width = width + 5);
	for (name, (grade, weight)) in assignment_names.iter().zip(grades.iter().zip(weights.iter())) {
		let fmt_name = format!("{}:", name);

		println!("{:<width$} {:<8.2} {:>8.2}", fmt_name, grade, weight, width = width + 5);
	}
	println!("{:<width$} {:<.2}", "Your current grade:", cur_grade, width = width + 5);
	println!("{:<width$} {:<.2}", "Your minimum grade:", min_grade, width = width + 5);
	println!("{:<width$} {:<.2}", "Your maximum grade:", max_grade, width = width + 5);
	println!("{:<width$} {:<.2}", "Ungraded points you can earn:", max_points, width = width + 5);

	Ok(())
}

fn get_login_credentials() -> Result<(String, String), io::Error> {
	print!("username: ");
	io::stdout().flush().unwrap();

	let mut username = String::new();
	io::stdin().read_line(&mut username)?;
	if username.is_empty() {
		println!();
		return Err(io::Error::new(ErrorKind::InvalidInput, "no username provided!"));
	}

	let password = match rpassword::read_password_from_tty(Some("password: ")) {
		Ok(s) if s.is_empty() => {
			println!();
			return Err(io::Error::new(ErrorKind::InvalidInput, "no password provided"));
		},
		Ok(s) => s,
		Err(e) => {
			println!();
			return Err(e);
		},
	};

	Ok((username, password))
}

fn lookup_course(course: &str) -> Result<u32, &str> {
	match course.to_lowercase().as_str() {
		"fundies2" | "f2" | "cs2510" => Ok(129),
		"fundies2accel" | "f2accel" | "f2a" | "cs2510a" => Ok(126),
		_ => Err("Course not found"),
	}
}

// returns assignment names, grades, weights, ungraded weights in that order
async fn scrape_grades(client: &reqwest::Client, _url: &str) -> Result<(Vec<String>, Vec<f64>, Vec<f64>, Vec<f64>), Box<dyn Error>> {
	let assignments = client.get("https://handins.ccs.neu.edu/courses/126/assignments/")
		.header("Referer", "https://handins.ccs.neu.edu/")
		.send().await?
		.text().await?;

	let doc = Document::from(assignments.as_str());

	let assignment_names: Vec<String> = doc
		.find(Name("tbody"))
		.next().unwrap()
		.find(Name("tr"))
		.map(|node| node.find(Name("td")).into_selection().first().unwrap()
			.first_child().unwrap()
			.first_child().unwrap().as_text().unwrap().to_string())
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

	let ungraded: Vec<f64> = grades.iter().filter(|vals| vals.len() == 1).map(|weight| weight[0]).collect();

	let graded: Vec<Vec<f64>> = grades.into_iter().filter(|vals| vals.len() > 1).collect();

	let weights: Vec<f64> = graded.iter().map(|vals| vals[0]).collect();
	let grades: Vec<f64> = graded.iter().map(|vals| vals[1]).collect();

	Ok((assignment_names, grades, weights, ungraded))
}

fn calculate_grade(grades: &Vec<f64>, weights: &Vec<f64>, ungraded_weights: &Vec<f64>) -> (f64, f64, f64, f64) {
	assert!(grades.len() == weights.len());

	let total_weight: f64 = weights.iter().sum();

	let scaled_grade = grades.iter().zip(weights.iter())
		.fold(0.0, |sum, grade_pair| grade_pair.0 * grade_pair.1 + sum);

	let future_weights: f64 = ungraded_weights.iter().sum();
	let optimistic_grade = scaled_grade + 100.0 * (100.0 - total_weight);

	(
		scaled_grade / total_weight, // your current grade
		scaled_grade / 100.0, 		 // your minimum grade
		optimistic_grade / 100.0, 	 // maximum possible grade for the course
		// most points you can earn from ungraded assignments
		// delta (max possible grade from ungraded assignments, current grade)
		(scaled_grade + 100.0 * future_weights) / (total_weight + future_weights) - (scaled_grade / total_weight),
	)
}
