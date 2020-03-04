use chrono::naive::NaiveDate as Date;
use failure::format_err;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

struct DateRange {
	start: Date,
	end: Option<Date>,
}

impl DateRange {
	fn to_resume_string(&self) -> String {
		if let Some(end) = self.end {
			format!("{} - {}", self.start.format("%b, %Y"), end.format("%b, %Y"))
		} else {
			format!("{} - Current", self.start.format("%b, %Y"))
		}
	}
}

impl std::fmt::Display for DateRange {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(end) = self.end {
			write!(f, "{}~{}", self.start.format("%Y-%m"), end.format("%Y-%m"))
		} else {
			write!(f, "{}~", self.start.format("%Y-%m"))
		}
	}
}

impl std::str::FromStr for DateRange {
	type Err = failure::Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let p: Vec<_> = s.split("~").collect();
		if p.len() != 2 {
			Err(format_err!("A date range should have 2 and only 2 dates"))
		} else {
			Ok(DateRange {
				start: Date::parse_from_str(&format!("{}-01", p[0]), "%Y-%m-%d")
					.unwrap(),
				end: if p[1] == "" {
					None
				} else {
					Some(Date::parse_from_str(
						&format!("{}-01", p[1]),
						"%Y-%m-%d",
					)
					.unwrap())
				},
			})
		}
	}
}

impl Serialize for DateRange {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		s.serialize_str(&self.to_string())
	}
}

impl<'a> Deserialize<'a> for DateRange {
	fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Self, D::Error> {
		let s = String::deserialize(d)?;
		s.parse().map_err(serde::de::Error::custom)
	}
}

#[derive(Serialize, Deserialize)]
enum Degree {
	BS,
	MS,
	PhD,
}

impl Degree {
	fn to_resume_string(&self) -> String {
		match self {
			Self::BS => "Bachelor of Science".into(),
			Self::MS => "Master of Science".into(),
			Self::PhD => "PhD".into(),
		}
	}
}

#[derive(Serialize, Deserialize)]
struct Education {
	institution: String,
	degree: Degree,
	major: String,
	duration: DateRange,
	#[serde(skip_serializing_if = "Option::is_none")]
	gpa: Option<f32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	courses: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct Experience {
	company: String,
	duration: DateRange,
	#[serde(skip_serializing_if = "Option::is_none")]
	description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Contact {
	#[serde(rename = "type")]
	type_: String,
	value: String,
}

#[derive(Serialize, Deserialize)]
struct Person {
	name: String,
	contacts: Vec<Contact>,
	educations: Vec<Education>,
	experiences: Vec<Experience>,
}

use askama::Template;

struct ContactParams {
	value: String,
	icon: Option<String>,
	link: Option<String>,
}
#[derive(Template)]
#[template(path = "resume.html", escape = "none")]
struct ResumeParams<'a> {
	name: &'a str,
	contacts: Vec<ContactParams>,
	educations: &'a [Education],
}

fn build_params<'a>(p: &'a Person) -> ResumeParams<'a> {
	let mut c = Vec::new();
	for i in p.contacts.iter() {
		c.push(ContactParams {
			link: match i.type_.as_str() {
				"github" => Some(format!("https://github.com/{}", i.value)),
				"email" => Some(format!("mailto:{}", i.value)),
				_ => None,
			},
			icon: match i.type_.as_str() {
				"github" => Some(
					"icons/github.svg".into(),
				),
				"email" => Some(
					"icons/mail.svg".into()
				),
				_ => None,
			},
			value: i.value.clone(),
		});
	}

	ResumeParams {
		name: &p.name,
		contacts: c,
		educations: p.educations.as_slice(),
	}
}

fn main() {
	let args = clap::App::new("resume").arg(clap::Arg::with_name("input").required(true));
	let f = std::fs::read(args.get_matches().value_of("input").unwrap()).unwrap();
	let r = serde_yaml::from_slice::<Person>(f.as_slice()).unwrap();
	eprintln!("{}", serde_yaml::to_string(&r).unwrap());

	let resume = build_params(&r);
	println!("{}", resume.render().unwrap());
}
