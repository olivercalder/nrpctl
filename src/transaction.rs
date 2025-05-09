use anyhow::{Error, Result};

/// A transaction is used to execute a series of functions, each of which returns a callback which
/// is capable of rolling back any side-effects which the function causes. When pushed to the
/// transaction, each function is executed immediately, potentially causing immediate side-effects.
/// If calling the function results in an error, each callback corresponding to a function which
/// has previously succeeded is called in reverse order, thus undoing all previously-enacted
/// changes, and the initial error is returned, chained to any errors which resulted from any
/// failed restore callback.
pub struct Transaction {
    stack: Vec<Box<dyn FnOnce() -> Result<String>>>,
}

impl Transaction {
    /// Create a new transaction.
    pub fn new() -> Transaction {
        Transaction { stack: Vec::new() }
    }

    /// Immediately call the given function. If it succeeds, it should return a callback which will
    /// roll back any changes which the function caused; that callback is added to the transaction.
    /// If it errors, then call the callback for every previous function which succeeded, and
    /// return the original error, chained with any errors which resulted from any failed
    /// rollbacks.
    pub fn do_or_rollback(
        &mut self,
        f: impl FnOnce() -> Result<Box<dyn FnOnce() -> Result<String>>>,
    ) -> Result<()> {
        match f() {
            Ok(callback) => {
                self.stack.push(callback);
                Ok(())
            }
            Err(e) => Err(self.rollback(e)),
        }
    }

    fn rollback(&mut self, cause: Error) -> Error {
        self.stack.drain(..).rev().fold(cause, |err, f| match f() {
            Ok(desc) => {
                println!("{}", desc);
                err
            }
            Err(restore_err) => restore_err.context(err),
        })
    }
}

// TODO: add transactional file write, modeled after the restore callbacks from write_nginx_site
// and delete_nginx_site, and use for backup_and_write_config as well.
