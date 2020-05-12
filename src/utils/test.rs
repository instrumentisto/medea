use futures::{future, future::Either, Future};
use medea_client_api_proto::stats::HighResTimeStamp;
use std::time::{Duration, SystemTime};
use tokio::time::delay_for;

pub async fn wait_or_fail<T>(
    fut: impl Future<Output = T>,
    dur: Duration,
) -> Result<T, ()> {
    let result = future::select(Box::pin(fut), Box::pin(delay_for(dur))).await;
    match result {
        Either::Left((res, _)) => Ok(res),
        Either::Right(_) => Err(()),
    }
}

pub fn timestamp(time: SystemTime) -> HighResTimeStamp {
    HighResTimeStamp(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64,
    )
}
