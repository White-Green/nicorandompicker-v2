use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, RngExt};
use std::collections::HashSet;
use std::error::Error;

const MAX_SEARCH_REQUESTS: usize = 10;
const INITIAL_TOTAL_COUNT: usize = 1600;
const SNAPSHOT_PAGE_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn choose<R: Rng>(rng: &mut R) -> SortDirection {
        *[SortDirection::Asc, SortDirection::Desc].choose(rng).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPage<R> {
    pub total_count: usize,
    pub result: Vec<R>,
}

pub trait ContentId {
    fn content_id(&self) -> &str;
}

pub trait SearchBackend<Q> {
    type Error: Error;
    type SortField: 'static;
    const SORT_SPECS: &'static [Self::SortField];
    type Result: ContentId;

    async fn search(
        &mut self,
        query: &Q,
        sort_field: &Self::SortField,
        sort_direction: SortDirection,
        limit: usize,
        offset: usize,
    ) -> Result<SearchPage<Self::Result>, Self::Error>;
}

#[tracing::instrument(skip(backend, query, rng))]
pub async fn collect_video_ids<B, Q, R>(mut backend: B, query: Q, count: usize, mut rng: R) -> Result<Vec<B::Result>, B::Error>
where
    B: SearchBackend<Q>,
    R: Rng,
{
    tracing::info!("search {count} videos");
    assert!(!B::SORT_SPECS.is_empty());
    if count == 0 {
        return Ok(Vec::new());
    }

    let random_request_count = MAX_SEARCH_REQUESTS.saturating_sub(1);
    let mut total_count = INITIAL_TOTAL_COUNT;
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for attempt in 0..random_request_count {
        let remaining = count - result.len();
        let remaining_requests = random_request_count - attempt;
        let sort = B::SORT_SPECS.choose(&mut rng).unwrap();
        let direction = SortDirection::choose(&mut rng);
        let max_limit = SNAPSHOT_PAGE_LIMIT.min(total_count.max(1));
        let min_limit = 3.min(max_limit);
        let limit = (remaining * 3).div_ceil(remaining_requests.max(1) * 2).clamp(min_limit, max_limit);
        let offset = rng.random_range(0..=total_count.saturating_sub(limit).min(100000));
        let page = backend.search(&query, sort, direction, limit, offset).await?;
        tracing::info!("found {} videos", page.result.len());
        total_count = page.total_count;

        if total_count < ((count as f64) * 1.2) as usize {
            let sort = B::SORT_SPECS.choose(&mut rng).unwrap();
            let direction = SortDirection::choose(&mut rng);
            let limit = count.min(total_count).min(SNAPSHOT_PAGE_LIMIT);
            let offset = rng.random_range(0..=total_count.saturating_sub(limit));
            let mut page = backend.search(&query, sort, direction, limit, offset).await?;
            tracing::info!("found {} videos", page.result.len());
            page.result.shuffle(&mut rng);
            page.result.truncate(count);
            return Ok(page.result);
        }

        for r in page.result {
            if seen.insert(r.content_id().to_owned()) {
                result.push(r);
                if result.len() >= count {
                    break;
                }
            }
        }

        if result.len() >= count {
            result.shuffle(&mut rng);
            result.truncate(count);
            return Ok(result);
        }
    }

    if result.len() < count {
        let sort = B::SORT_SPECS.choose(&mut rng).unwrap();
        let direction = SortDirection::choose(&mut rng);
        let limit = count.min(total_count).min(SNAPSHOT_PAGE_LIMIT);
        let offset = rng.random_range(0..=total_count.saturating_sub(limit));
        let mut page = backend.search(&query, sort, direction, limit, offset).await?;
        tracing::info!("found {} videos", page.result.len());
        page.result.shuffle(&mut rng);
        for r in page.result {
            if seen.insert(r.content_id().to_owned()) {
                result.push(r);
                if result.len() >= count {
                    break;
                }
            }
        }
    }

    result.shuffle(&mut rng);
    result.truncate(count);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::rc::Rc;
    use std::sync::atomic;
    use std::sync::atomic::AtomicUsize;

    #[derive(Debug, thiserror::Error)]
    #[error("mock search failed")]
    struct MockError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MockVideo {
        content_id: String,
        fields: [usize; 5],
    }

    impl ContentId for MockVideo {
        fn content_id(&self) -> &str {
            &self.content_id
        }
    }

    struct MockBackend {
        counter: Rc<AtomicUsize>,
        videos: Vec<MockVideo>,
    }

    impl SearchBackend<()> for MockBackend {
        type Error = MockError;
        type SortField = usize;
        const SORT_SPECS: &'static [Self::SortField] = &[0, 1, 2, 3, 4];
        type Result = MockVideo;

        async fn search(
            &mut self,
            _query: &(),
            &sort_field: &Self::SortField,
            sort_direction: SortDirection,
            limit: usize,
            offset: usize,
        ) -> Result<SearchPage<Self::Result>, Self::Error> {
            let count = self.counter.fetch_add(1, atomic::Ordering::Relaxed);
            if count > MAX_SEARCH_REQUESTS * 2 {
                return Err(MockError);
            }
            self.videos.sort_unstable_by_key(|video| video.fields[sort_field]);
            if sort_direction == SortDirection::Desc {
                self.videos.reverse();
            }
            let result = self
                .videos
                .get(offset..)
                .map_or_else(Vec::new, |pages| pages[..limit.min(pages.len())].to_vec());
            Ok(SearchPage {
                total_count: self.videos.len(),
                result,
            })
        }
    }

    fn arb_video() -> impl Strategy<Value = MockVideo> {
        static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
        (
            0..1_000_000usize,
            0..1_000_000usize,
            0..1_000_000usize,
            0..1_000_000usize,
            0..1_000_000usize,
        )
            .prop_map(|(f1, f2, f3, f4, f5)| MockVideo {
                content_id: format!("sm{}", ID_COUNTER.fetch_add(1, atomic::Ordering::Relaxed)),
                fields: [f1, f2, f3, f4, f5],
            })
    }

    proptest! {
        #[test]
        fn collect_video_ids_uses_few_searches(
            count in 0usize..=100,
            seed in any::<u64>(),
            videos in prop::collection::vec(arb_video(), 0..=200),
        ) {
            let counter = Rc::new(AtomicUsize::new(0));
            let hit_count = videos.len();
            let backend = MockBackend {
                counter: Rc::clone(&counter),
                videos: videos.clone(),
            };

            let result = pollster::block_on(collect_video_ids(backend, (), count, StdRng::seed_from_u64(seed)));

            prop_assert!(result.is_ok());
            let result = result.unwrap();

            let unique_count = result
                .iter()
                .map(|video| video.content_id())
                .collect::<HashSet<_>>()
                .len();
            prop_assert_eq!(result.len(), count.min(hit_count));
            prop_assert_eq!(unique_count, result.len());
            prop_assert!(counter.load(atomic::Ordering::Relaxed) <= MAX_SEARCH_REQUESTS);
        }

        #[test]
        fn collect_video_ids_returns_requested_count_when_enough_results_exist(
            count in 1usize..=100,
            seed in any::<u64>(),
            extra in 0usize..=100,
        ) {
            let videos = (0..count + extra)
                .map(|id| MockVideo {
                    content_id: format!("sm{id}"),
                    fields: [id, id * 2, id * 3, id * 5, id * 7],
                })
                .collect();
            let counter = Rc::new(AtomicUsize::new(0));
            let backend = MockBackend {
                counter: Rc::clone(&counter),
                videos,
            };

            let result = pollster::block_on(collect_video_ids(backend, (), count, StdRng::seed_from_u64(seed))).unwrap();
            let unique_count = result
                .iter()
                .map(|video| video.content_id())
                .collect::<HashSet<_>>()
                .len();

            prop_assert_eq!(result.len(), count);
            prop_assert_eq!(unique_count, count);
            prop_assert!(counter.load(atomic::Ordering::Relaxed) <= MAX_SEARCH_REQUESTS);
        }
    }
}
