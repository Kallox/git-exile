//! Ejecuta trabajos (normalmente comandos git) en un hilo aparte
//! para que la interfaz no se bloquee.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui;

type Job<T> = Box<dyn FnOnce() -> T + Send>;

pub struct Worker<T> {
    jobs: Sender<Job<T>>,
    results: Receiver<T>,
    pending: usize,
}

impl<T: Send + 'static> Worker<T> {
    pub fn new(ctx: egui::Context) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job<T>>();
        let (result_tx, results) = mpsc::channel();
        thread::Builder::new()
            .name("git-worker".into())
            .spawn(move || {
                for job in job_rx {
                    if result_tx.send(job()).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("no se pudo crear el hilo de trabajo");
        Self {
            jobs,
            results,
            pending: 0,
        }
    }

    /// Encola un trabajo. Los trabajos se ejecutan en orden.
    pub fn spawn(&mut self, job: impl FnOnce() -> T + Send + 'static) {
        if self.jobs.send(Box::new(job)).is_ok() {
            self.pending += 1;
        }
    }

    /// Resultados terminados desde la última llamada.
    pub fn poll(&mut self) -> Vec<T> {
        let done: Vec<T> = self.results.try_iter().collect();
        self.pending -= done.len();
        done
    }

    pub fn is_busy(&self) -> bool {
        self.pending > 0
    }
}
