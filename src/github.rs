use crate::Project;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use futures::FutureExt;
use hubcaps::Github;
type Result<T> = ::std::result::Result<T, failure::Error>;

pub(crate) fn get_user_projects_from_github(
	username: &str,
	ignore_forks: bool,
	token: Option<String>,
) -> Result<impl Stream<Item = hubcaps::Result<Project>>> {
	let gh = Github::new("briefly/0.0", token.map(hubcaps::Credentials::Token))?;
	let gh2 = gh.clone();
	let opts = hubcaps::repositories::UserRepoListOptions::builder()
		.repo_type(hubcaps::repositories::Type::All)
		.build();
	Ok(gh
		.user_repos(username)
		.iter(&opts)
		.try_filter_map(move |v| {
			if !ignore_forks {
				futures::future::ok(Some(v))
			} else {
				if v.fork {
					futures::future::ok(None)
				} else {
					futures::future::ok(Some(v))
				}
			}
		})
		.and_then(move |r| {
			let gh2 = gh2.clone();
			async move {
				use futures::future::join;
				let (t, langs) = join(r.topics(gh2.clone()), r.languages(gh2.clone())).await;
				let t = t?;
				let langs = langs?;
				let total_bytes = langs.values().sum::<i64>() as f64;
				let mut langs: Vec<_> = langs
					.into_iter()
					.map(|(k, v)| crate::LanguageStat {
						language: k,
						percentage: ((v as f64) / total_bytes * 100.0).into(),
					})
					.collect();
				langs.sort_unstable_by_key(|v| ::std::cmp::Reverse(v.percentage));
				Ok((r, t.names, langs))
			}
		})
		.map_ok(|(r, tags, languages)| Project {
			name: r.name,
			description: r.description,
			contributions: None,
			url: Some(r.html_url.parse().unwrap()),
			stars: Some(r.stargazers_count),
			forks: Some(r.forks_count),
			active: Some(!r.archived),
			owner: Some(r.owner.login),
			commits: None,
			additions: None,
			deletions: None,
			languages,
			tags,
			role: Some(crate::ProjectRole::Owner),
		}))
}

pub(crate) fn get_projects_info_from_github<'a, I>(
	repos: I,
	github_username: Option<&'a str>,
	token: Option<String>,
) -> Result<impl Stream<Item = hubcaps::Result<Project>> + 'a>
where
	I: IntoIterator,
	<I as IntoIterator>::Item: AsRef<str>,
{
	#[derive(Clone, Copy)]
	struct Contribution {
		additions: u64,
		deletions: u64,
		commits: u64,
	}
	impl ::std::ops::Add for Contribution {
		type Output = Self;
		fn add(self, rhs: Self) -> Self {
			Contribution {
				additions: self.additions + rhs.additions,
				deletions: self.deletions + rhs.deletions,
				commits: self.commits + rhs.commits,
			}
		}
	}
	impl From<&hubcaps::repositories::Week> for Contribution {
		fn from(w: &hubcaps::repositories::Week) -> Self {
			Self {
				additions: w.additions,
				deletions: w.deletions,
				commits: w.commits,
			}
		}
	}
	let gh = Github::new("briefly/0.0", token.map(hubcaps::Credentials::Token))?;
	let st: futures::stream::FuturesUnordered<_> = repos
		.into_iter()
		.filter_map(|v| {
			if let &[o, r] = v.as_ref().split("/").take(2).collect::<Vec<_>>().as_slice() {
				Some((o.to_owned(), r.to_owned()))
			} else {
				None
			}
		})
		.map(move |(o, r)| {
			let gh = gh.clone();
			async move {
				use futures::future::{join, join3};
				let r = gh.repo(o, r).get().await?;
				let topics = r.topics(gh.clone());
				let languages = r.languages(gh.clone());
				// Fetch info about user contributions
				let (stats, topics, languages) = if let Some(github_username) = github_username
				{
					use futures::future::ready;
					join3(
						gh.repo(&r.owner.login, &r.name)
							.contributor_statistics()
							.iter()
							.filter_map(|v| {
								ready(if let Ok(v) = v {
									if v.author.login == github_username {
										Some(v)
									} else {
										None
									}
								} else {
									None
								})
							})
							.collect::<Vec<_>>()
							.map(Option::Some),
						topics,
						languages,
					)
					.await
				} else {
					let (topics, languages) = join(topics, languages).await;
					(None, topics, languages)
				};
				let languages = languages?;
				let total_bytes = languages.values().sum::<i64>() as f64;
				let mut languages: Vec<_> = languages
					.into_iter()
					.map(|(k, v)| crate::LanguageStat {
						language: k,
						percentage: ((v as f64) / total_bytes * 100.0).into(),
					})
					.collect();
				languages.sort_unstable_by_key(|v| ::std::cmp::Reverse(v.percentage));
				let (additions, deletions, commits) = if let Some(stats) = stats {
					let zero = Contribution {
						additions: 0,
						deletions: 0,
						commits: 0,
					};
					let contributions = stats
						.iter()
						.map(|v| {
							v.weeks
								.iter()
								.map(Into::into)
								.fold(zero, ::std::ops::Add::add)
						})
						.fold(zero, ::std::ops::Add::add);
					(
						Some(contributions.additions),
						Some(contributions.deletions),
						Some(contributions.commits),
					)
				} else {
					(None, None, None)
				};

				Ok(Project {
					name: r.name,
					description: r.description,
					contributions: None,
					url: Some(r.html_url.parse().unwrap()),
					stars: Some(r.stargazers_count),
					forks: Some(r.forks_count),
					active: Some(!r.archived),
					owner: Some(r.owner.login),
					additions,
					deletions,
					commits,
					languages,
					tags: topics?.names,
					role: Some(crate::ProjectRole::Contributor),
				})
			}
		})
		.collect();
	Ok(st.boxed())
}
