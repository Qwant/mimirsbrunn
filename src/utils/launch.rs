// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
//
// LICENCE: This program is free software; you can redistribute it
// and/or modify it under the terms of the GNU Affero General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public
// License along with this program. If not, see
// <http://www.gnu.org/licenses/>.
//
// Stay tuned using
// twitter @navitia
// IRC #navitia on freenode
// https://groups.google.com/d/forum/navitia
// www.navitia.io

use futures::future::Future;
use lazy_static::lazy_static;
use tokio::runtime;
use tracing::error;

use super::logger::logger_init;

lazy_static! {
    pub static ref DEFAULT_NB_THREADS: String = num_cpus::get().to_string();
}

// Ensures the logger is initialized prior to launching a function, and also making sure the logger
// is flushed at the end. Whatever is returned by the main function is forwarded out.
pub async fn wrapped_launch_async<F, Fut>(run: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    let guard = logger_init().map_err(Box::new)?;

    let res = if let Err(err) = run().await {
        // To revisit when rust #58520 is resolved
        // for cause in err.chain() {
        //     error!("{}", cause);
        // }
        if let Some(source) = err.source() {
            error!("{}", source);
        }
        Err(err)
    } else {
        Ok(())
    };

    // Ensure the logger persists until the future is resolved
    // and is flushed before the process exits.
    drop(guard);
    res
}

// Ensures the logger is initialized prior to launching a function, and also making sure the logger
// is flushed at the end. Whatever is returned by the main function is forwarded out.
pub fn launch_with_runtime<F>(
    nb_threads: Option<usize>,
    run: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    let guard = logger_init().map_err(Box::new)?;

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(nb_threads.unwrap_or_else(num_cpus::get))
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime.");

    let res = if let Err(err) = runtime.block_on(run) {
        if let Some(source) = err.source() {
            error!("{}", source);
        }
        Err(err)
    } else {
        Ok(())
    };

    // Ensure the logger persists until the future is resolved
    // and is flushed before the process exits.
    drop(guard);
    res
}

pub async fn launch_async<F, Fut>(run: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    let res = if let Err(err) = run().await {
        // To revisit when rust #58520 is resolved
        // for cause in err.chain() {
        //     eprintln!("{}", cause);
        // }
        if let Some(source) = err.source() {
            eprintln!("{}", source);
        }
        Err(err)
    } else {
        Ok(())
    };

    res
}
