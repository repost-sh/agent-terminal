use super::Daemon;
use crate::protocol::{BatchOutcome, ErrorBody, Reply, Request, Response, request};

impl Daemon {
    pub(super) fn batch(&self, batch: request::Batch) -> Reply {
        let mut outcomes = Vec::with_capacity(batch.steps.len());
        let mut failed = false;
        for step in batch.steps {
            let response = if batch.bail && failed { None } else { Some(self.batch_step(&step.args, step.request)) };
            failed |= response.as_ref().is_some_and(|response| response.exit_code() != 0);
            outcomes.push(BatchOutcome { args: step.args, response });
        }
        Reply::Batch(outcomes)
    }

    fn batch_step(&self, args: &[String], request: Request) -> Response {
        if matches!(request, Request::Stop | Request::StopIfNoTerminals) {
            return Response::Err(ErrorBody::failure("stopping the daemon cannot be part of a batch"));
        }
        let logged = self.about_to_handle(args.to_vec(), &request);
        let handled = self.handle(request, Some(args));
        if let Some(logged) = logged {
            self.log(logged, &handled);
        }
        match handled {
            Ok(reply) => Response::Ok(reply),
            Err(failure) => Response::Err(failure),
        }
    }
}
