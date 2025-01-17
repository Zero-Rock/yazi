use yazi_config::{PLUGIN, plugin::MAX_PREWORKERS};
use yazi_fs::{File, Files, SortBy};

use super::Tasks;
use crate::manager::Mimetype;

impl Tasks {
	pub fn fetch_paged(&self, paged: &[File], mimetype: &Mimetype) {
		let mut loaded = self.scheduler.prework.loaded.lock();
		let mut tasks: [Vec<_>; MAX_PREWORKERS as usize] = Default::default();
		for f in paged {
			let mime = mimetype.by_file(f).unwrap_or_default();
			let factors = |s: &str| match s {
				"mime" => !mime.is_empty(),
				"dummy" => f.cha.is_dummy(),
				_ => false,
			};

			for g in PLUGIN.fetchers(&f.url, mime, factors) {
				match loaded.get_mut(&f.url) {
					Some(n) if *n & (1 << g.idx) != 0 => continue,
					Some(n) => *n |= 1 << g.idx,
					None => _ = loaded.insert(f.url_owned(), 1 << g.idx),
				}
				tasks[g.idx as usize].push(f.clone());
			}
		}

		drop(loaded);
		for (i, tasks) in tasks.into_iter().enumerate() {
			if !tasks.is_empty() {
				self.scheduler.fetch_paged(&PLUGIN.fetchers[i], tasks);
			}
		}
	}

	pub fn preload_paged(&self, paged: &[File], mimetype: &Mimetype) {
		let mut loaded = self.scheduler.prework.loaded.lock();
		for f in paged {
			let mime = mimetype.by_file(f).unwrap_or_default();
			for p in PLUGIN.preloaders(&f.url, mime) {
				match loaded.get_mut(&f.url) {
					Some(n) if *n & (1 << p.idx) != 0 => continue,
					Some(n) => *n |= 1 << p.idx,
					None => _ = loaded.insert(f.url_owned(), 1 << p.idx),
				}
				self.scheduler.preload_paged(p, f);
			}
		}
	}

	pub fn prework_affected(&self, affected: &[File], mimetype: &Mimetype) {
		let mask = PLUGIN.fetchers_mask();
		{
			let mut loaded = self.scheduler.prework.loaded.lock();
			for f in affected {
				loaded.get_mut(&f.url).map(|n| *n &= mask);
			}
		}

		self.fetch_paged(affected, mimetype);
		self.preload_paged(affected, mimetype);
	}

	pub fn prework_sorted(&self, targets: &Files) {
		if targets.sorter().by != SortBy::Size {
			return;
		}

		let targets: Vec<_> = {
			let loading = self.scheduler.prework.size_loading.read();
			targets
				.iter()
				.filter(|f| f.is_dir() && !targets.sizes.contains_key(f.urn()) && !loading.contains(&f.url))
				.map(|f| &f.url)
				.collect()
		};
		if targets.is_empty() {
			return;
		}

		let mut loading = self.scheduler.prework.size_loading.write();
		for &target in &targets {
			loading.insert(target.clone());
		}

		self.scheduler.prework_size(targets);
	}
}
