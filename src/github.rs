use crate::Project;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use futures::FutureExt;
use octocrab::OctocrabBuilder;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) async fn get_user_projects_from_github(
	ignore_forks: bool,
	token: Option<String>,
) -> anyhow::Result<Vec<Project>> {
	use octocrab::service::middleware;
	let connector = hyper_rustls::HttpsConnectorBuilder::new()
		.with_native_roots() // enabled the `rustls-native-certs` feature in hyper-rustls
		.https_only()
		.enable_http1()
		.build();

	// When running from github actions
	let token = token.unwrap_or_else(|| std::env::var("GITHUB_TOKEN").unwrap());

	let client = hyper::Client::builder().build(connector);
	let gh = OctocrabBuilder::new_empty()
		.with_service(client)
		.with_layer(&middleware::base_uri::BaseUriLayer::new(
			http::Uri::from_static("https://api.github.com"),
		))
		.with_layer(&middleware::extra_headers::ExtraHeadersLayer::new(
			Arc::new(vec![
				(http::header::USER_AGENT, "briefly/0.0".parse().unwrap()),
				(
					http::header::AUTHORIZATION,
					format!("Bearer {}", token).parse().unwrap(),
				),
			]),
		))
		.with_auth(octocrab::AuthState::None)
		.build()?;
	let mut repos = gh
		.current()
		.list_repos_for_authenticated_user()
		.send()
		.await?;
	let mut ret = Vec::new();
	loop {
		for repo in &repos {
			if ignore_forks && repo.fork.unwrap_or(false) {
				continue;
			}
			let mut languages = Vec::new();
			if let Some(languages_url) = &repo.languages_url {
				use hyper::body::HttpBody;
				let mut languages_resp = gh._get(languages_url.to_string()).await?;
				let value: HashMap<String, u64> = serde_json::from_slice(
					&languages_resp
						.body_mut()
						.data()
						.await
						.transpose()?
						.unwrap_or_default(),
				)?;
				let mut total = 0f64;
				for (_, v) in &value {
					total += *v as f64;
				}
				for (k, v) in &value {
					languages.push(crate::LanguageStat {
						language: k.clone(),
						percentage: (*v as f64 / total * 100.).into(),
					});
				}
			}
			languages.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());
			ret.push(Project {
				name: repo.name.clone(),
				description: repo.description.clone(),
				contributions: None,
				url: repo.html_url.clone(),
				stars: repo.stargazers_count.map(|v| v as u64),
				forks: repo.forks_count.map(|v| v as u64),
				active: repo.archived.map(|v| !v),
				owner: Some(repo.owner.clone().map(|o| o.login).unwrap_or_default()),
				commits: None,
				additions: None,
				deletions: None,
				languages,
				tags: repo.topics.clone().unwrap_or_default(),
				role: Some(crate::ProjectRole::Owner),
			})
		}
		let Some(next_repos) = gh.get_page(&repos.next).await? else {
            break;
        };
		repos = next_repos;
	}
	Ok(ret)
}

pub(crate) async fn get_projects_info_from_github<'a, I>(
	repos: I,
	token: Option<String>,
	user: Option<String>,
) -> anyhow::Result<Vec<Project>>
where
	I: IntoIterator,
	<I as IntoIterator>::Item: AsRef<str>,
{
	use octocrab::service::middleware;
	let connector = hyper_rustls::HttpsConnectorBuilder::new()
		.with_native_roots() // enabled the `rustls-native-certs` feature in hyper-rustls
		.https_only()
		.enable_http1()
		.build();

	// When running from github actions
	let token = token.unwrap_or_else(|| std::env::var("GITHUB_TOKEN").unwrap());
	let client = hyper::Client::builder().build(connector);
	let gh = OctocrabBuilder::new_empty()
		.with_service(client)
		.with_layer(&middleware::base_uri::BaseUriLayer::new(
			http::Uri::from_static("https://api.github.com"),
		))
		.with_layer(&middleware::extra_headers::ExtraHeadersLayer::new(
			Arc::new(vec![
				(http::header::USER_AGENT, "briefly/0.0".parse().unwrap()),
				(
					http::header::AUTHORIZATION,
					format!("Bearer {}", token).parse().unwrap(),
				),
			]),
		))
		.with_auth(octocrab::AuthState::None)
		.build()?;
	let current_user = if let Some(user) = user {
		user
	} else {
		gh.current().user().await?.login
	};
	let st: futures::stream::FuturesUnordered<_> = repos
		.into_iter()
		.filter_map(|v| {
			if let &[o, r] = v.as_ref().split("/").take(2).collect::<Vec<_>>().as_slice() {
				Some((o.to_owned(), r.to_owned()))
			} else {
				None
			}
		})
		.map(|(o, r)| {
			let gh = gh.clone();
			let current_user = current_user.clone();
			async move {
				let r = gh.repos(o, r).get().await?;
				let mut languages = Vec::new();
				if let Some(languages_url) = &r.languages_url {
					use hyper::body::HttpBody;
					let mut languages_resp = gh._get(languages_url.to_string()).await?;
					let value: HashMap<String, u64> = serde_json::from_slice(
						&languages_resp
							.body_mut()
							.data()
							.await
							.transpose()?
							.unwrap_or_default(),
					)?;
					let mut total = 0f64;
					for v in value.values() {
						total += *v as f64;
					}
					for (k, v) in &value {
						languages.push(crate::LanguageStat {
							language: k.clone(),
							percentage: (*v as f64 / total * 100.).into(),
						});
					}
				}
				languages.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

				let owner = r.owner.clone().unwrap().login;
				Result::<_, anyhow::Error>::Ok(Project {
					name: r.name,
					description: r.description,
					contributions: None,
					url: r.html_url.clone(),
					stars: r.stargazers_count.map(|v| v as _),
					forks: r.forks_count.map(|v| v as _),
					active: r.archived.map(|archived| !archived),
					owner: r.owner.clone().map(|o| o.login),
					additions: None,
					deletions: None,
					commits: None,
					languages,
					tags: r.topics.unwrap_or_default(),
					role: Some(if owner == current_user {
						crate::ProjectRole::Owner
					} else {
						crate::ProjectRole::Contributor
					}),
				})
			}
		})
		.collect();
	st.try_collect().await
}
