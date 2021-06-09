use chrono::{DateTime, Duration, FixedOffset, Local};

// represents an assignment with additional metadata from scraping: the
// name, relative link, if the assignment was graded, and its due date
#[derive(Debug, PartialEq)]
pub(crate) struct Assignment {
    pub(crate) name: String,
    pub(crate) id: i64,
    pub(crate) grade: Option<f64>,
    pub(crate) weight: f64,
    pub(crate) due_date: DateTime<FixedOffset>,
}

impl Assignment {
    pub fn new(
        name: String,
        id: i64,
        grade: Option<f64>,
        weight: f64,
        due_date: DateTime<FixedOffset>,
    ) -> Assignment {
        Assignment {
            name,
            id,
            grade,
            weight,
            due_date,
        }
    }

    pub fn late(&self) -> bool {
        let now = Local::now();
        now >= self.due_date
    }

    pub fn how_late(&self) -> Duration {
        let now = Local::now();
        now - self.due_date.with_timezone(&Local)
    }

    pub fn graded(&self) -> bool {
        self.grade.is_some()
    }

    pub fn submission_link(&self, course_id: i64) -> String {
        format!("https://handins.ccs.neu.edu/courses/{}/assignments/{}/submissions/new", course_id, self.id)
    }
}