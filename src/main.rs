use ::std::collections::{HashMap, HashSet};
use chrono::naive::NaiveDate as Date;
use failure::{format_err, Error};
use log::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
mod github;

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
				start: Date::parse_from_str(&format!("{}-01", p[0]), "%Y-%m-%d").unwrap(),
				end: if p[1] == "" {
					None
				} else {
					Some(Date::parse_from_str(&format!("{}-01", p[1]), "%Y-%m-%d").unwrap())
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
	projects: Vec<ProjectParam>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ProjectParam {
	Import(ProjectImport),
	Sort { order_by: ProjectSortOrder },
	ImportMode { import_mode: ProjectImportMode },
	Raw(Project),
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProjectImportMode {
	Whitelist,
	Combine,
}

impl Default for ProjectImportMode {
	fn default() -> Self {
		Self::Combine
	}
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
enum ProjectSortOrder {
	Stars,
	Forks,
	StarsThenForks,
	ForksThenStars,
	Manual,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "lowercase")]
enum ProjectImport {
	GitHub {
		#[serde(default)]
		ignore_forks: bool,
		#[serde(default)]
		repos: Option<Vec<String>>,
	},
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Project {
	name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	contributions: Option<String>,
	#[serde(
		with = "serde_option_display_fromstr",
		default,
		skip_serializing_if = "Option::is_none"
	)]
	url: Option<url::Url>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	stars: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	forks: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	active: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	owner: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	commits: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	additions: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	deletions: Option<u64>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	languages: Vec<String>,
}

mod serde_option_display_fromstr {
	pub(crate) fn deserialize<'de, D, T>(deser: D) -> Result<Option<T>, D::Error>
	where
		D: serde::Deserializer<'de>,
		T: ::std::str::FromStr,
		<T as ::std::str::FromStr>::Err: ::std::fmt::Display,
	{
		#[derive(Default)]
		struct Visitor<T>(::std::marker::PhantomData<T>);
		impl<'de, T> serde::de::Visitor<'de> for Visitor<T>
		where
			T: ::std::str::FromStr,
			<T as ::std::str::FromStr>::Err: ::std::fmt::Display,
		{
			type Value = Option<T>;
			fn expecting(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				write!(fmt, "a string")
			}
			fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
				v.parse()
					.map_err(serde::de::Error::custom)
					.map(Option::Some)
			}
		}

		deser.deserialize_str(Visitor::<T>(Default::default()))
	}

	pub(crate) fn serialize<S, T>(v: &Option<T>, ser: S) -> Result<S::Ok, S::Error>
	where
		S: serde::ser::Serializer,
		T: ::std::fmt::Display,
	{
		match v {
			Some(v) => ser.serialize_str(&v.to_string()),
			None => ser.serialize_none(),
		}
	}
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
	projects: Vec<Project>,
}

async fn build_params<'a>(p: &'a Person) -> Result<ResumeParams<'a>, Error> {
	use futures::stream::{StreamExt, TryStreamExt};
	let mut c = Vec::new();
	let mut github_username = None;
	for i in p.contacts.iter() {
		c.push(ContactParams {
			link: match i.type_.as_str() {
				"github" => {
					github_username = Some(&i.value);
					Some(format!("https://github.com/{}", i.value))
				}
				"email" => Some(format!("mailto:{}", i.value)),
				_ => None,
			},
			icon: match i.type_.as_str() {
				"github" => Some("icons/github.svg".into()),
				"email" => Some("icons/mail.svg".into()),
				_ => None,
			},
			value: i.value.clone(),
		});
	}

	let mut project_map = HashMap::new();
	let mut sort_order = None;
	let mut import_mode = ProjectImportMode::Combine;
	// Process project imports first
	for pi in p.projects.iter() {
		match pi {
			ProjectParam::Import(ProjectImport::GitHub {
				ignore_forks,
				repos: None,
			}) => {
				if let Some(github_username) = github_username {
					async_std::stream::Extend::extend(
						&mut project_map,
						github::get_user_projects_from_github(github_username, *ignore_forks)?
							.map_ok(|v| (v.name.clone(), v))
							.map_err(|e| {
								error!("Failed fetching from GitHub {}", e);
								e
							})
							.filter_map(|x| futures::future::ready(x.ok())),
					)
					.await;
				} else {
					error!("No github user name supplied");
				}
			}
			ProjectParam::Import(ProjectImport::GitHub {
				repos: Some(repos), ..
			}) => {
				async_std::stream::Extend::extend(
					&mut project_map,
					github::get_projects_info_from_github(repos)?
						.map_ok(|v| (v.name.clone(), v))
						.map_err(|e| {
							error!("Failed fetching from GitHub {}", e);
							e
						})
						.filter_map(|x| futures::future::ready(x.ok())),
				)
				.await;
			}
			ProjectParam::Sort { order_by } => {
				sort_order = Some(*order_by);
			}
			ProjectParam::ImportMode { import_mode: im } => {
				import_mode = *im;
			}
			_ => {}
		}
	}
	if sort_order.is_none() && import_mode == ProjectImportMode::Whitelist {
		sort_order = Some(ProjectSortOrder::Manual);
	}

	// Adding manually project entries
	for pi in p.projects.iter() {
		match pi {
			ProjectParam::Raw(p) => {
				if let Some(old) = project_map.get_mut(&p.name) {
					debug!("Merging project entry {}", p.name);
					if p.url.is_some() {
						old.url = p.url.clone();
					}
					if p.description.is_some() {
						old.description = p.description.clone();
					}
					if p.owner.is_some() {
						old.owner = p.owner.clone();
					}
					if p.contributions.is_some() {
						old.contributions = p.contributions.clone();
					}
				} else {
					project_map.insert(p.name.clone(), p.clone());
				}
			}
			_ => {}
		}
	}
	let mut projects = Vec::new();
	if let ProjectImportMode::Whitelist = import_mode {
		let raw_entries: Vec<_> = p
			.projects
			.iter()
			.filter_map(|p| match p {
				ProjectParam::Raw(p) => Some(&p.name),
				_ => None,
			})
			.collect();
		debug!("{:?}", raw_entries);
		projects = raw_entries
			.into_iter()
			.filter_map(|name| project_map.get(name).map(Clone::clone))
			.collect();
	}
	if let Some(sort_order) = sort_order {
		use ::std::cmp::Reverse;
		if ProjectImportMode::Whitelist != import_mode {
			projects = project_map.iter().map(|(_, v)| v.clone()).collect();
		}
		match sort_order {
			ProjectSortOrder::Stars => projects.sort_unstable_by_key(|v| Reverse(v.stars)),
			ProjectSortOrder::Forks => projects.sort_unstable_by_key(|v| Reverse(v.forks)),
			ProjectSortOrder::ForksThenStars => {
				projects.sort_unstable_by_key(|v| Reverse((v.forks, v.stars)))
			}
			ProjectSortOrder::StarsThenForks => {
				projects.sort_unstable_by_key(|v| Reverse((v.stars, v.forks)))
			}
			ProjectSortOrder::Manual if import_mode != ProjectImportMode::Whitelist => {
				let raw_entries: HashMap<_, _> = p
					.projects
					.iter()
					.filter_map(|p| match p {
						ProjectParam::Raw(p) => Some(&p.name),
						_ => None,
					})
					.enumerate()
					.map(|(i, v)| (v, i))
					.collect();
				projects
					.sort_unstable_by_key(|v| Reverse(raw_entries.get(&v.name).map(|v| *v)));
			}
			_ => {}
		}
	}
	debug!("{}", serde_yaml::to_string(&projects)?);

	Ok(ResumeParams {
		name: &p.name,
		contacts: c,
		educations: p.educations.as_slice(),
		projects,
	})
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init();
	let args = clap::App::new("resume").arg(clap::Arg::with_name("input").required(true));
	let f = std::fs::read(args.get_matches().value_of("input").unwrap()).unwrap();
	let r = serde_yaml::from_slice::<Person>(f.as_slice()).unwrap();
	debug!("{}", serde_yaml::to_string(&r).unwrap());

	let resume = build_params(&r).await?;
	println!("{}", resume.render().unwrap());
	Ok(())
}
