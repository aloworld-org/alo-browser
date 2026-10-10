/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conductor's one inbox: the window's orders and the network thread's
//! results, in the order they arrived (ADR 0041 § 2).
//!
//! The conductor waits on this and on nothing else — never on a server — and
//! never for longer than the nearest bound it keeps ([`crate::hold`]). So a
//! resize, a change of visibility or a close is carried out while a request
//! is in flight, however slowly its server answers.
//!
//! # Knowing the window has gone
//!
//! Every sender of orders gone is the window gone without saying so, and the
//! conductor closes every tab. With one inbox that can no longer be read off
//! the channel closing — the network thread sends into it too, and lives until
//! the conductor lets it go — so the window's senders, [`Orders`], share one
//! [`Last`], which says [`Arrival::Abandoned`] as the last of them goes.

use crate::message::Order;
use alo_renderer::fetch_exchange::Exchanged;
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::time::Instant;

/// Something for the conductor.
#[derive(Debug)]
pub enum Arrival {
    /// What the window asks.
    Order(Order),
    /// An exchange the network thread was handed, made.
    Exchanged(Box<Exchanged>),
    /// The network thread has gone without being asked to — which the lints
    /// make unreachable except by a panic — and makes nothing more.
    NetworkGone,
    /// Every sender of orders has gone: the window, without saying so.
    Abandoned,
}

/// Where the window sends the conductor orders.
#[derive(Debug, Clone)]
pub struct Orders {
    inbox: Sender<Arrival>,
    /// Shared by every copy, and dropped with the last.
    _last: Arc<Last>,
}

/// Says [`Arrival::Abandoned`] when it is dropped, which is when the last
/// [`Orders`] holding it is.
#[derive(Debug)]
struct Last(Sender<Arrival>);

impl Drop for Last {
    fn drop(&mut self) {
        // Nobody to tell is a conductor that has already finished.
        let _ = self.0.send(Arrival::Abandoned);
    }
}

/// An order sent to a conductor that has finished, which nobody will carry
/// out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finished;

impl fmt::Display for Finished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the conductor has finished")
    }
}

impl std::error::Error for Finished {}

impl Orders {
    /// Send the conductor `order`.
    ///
    /// # Errors
    ///
    /// [`Finished`] when the conductor has.
    pub fn send(&self, order: Order) -> Result<(), Finished> {
        self.inbox.send(Arrival::Order(order)).map_err(|_| Finished)
    }
}

/// A new inbox: the window's [`Orders`], a sender for the network thread's
/// results, and the conductor's end.
pub fn inbox() -> (Orders, Sender<Arrival>, Receiver<Arrival>) {
    let (send, receive) = channel();
    let orders = Orders {
        inbox: send.clone(),
        _last: Arc::new(Last(send.clone())),
    };
    (orders, send, receive)
}

/// Everything that has arrived — after waiting for one thing, until `until`
/// at the latest or for as long as it takes when there is no `until`. Empty
/// when `until` passed with nothing; [`None`] when every sender has gone.
pub fn arrivals(inbox: &Receiver<Arrival>, until: Option<Instant>) -> Option<Vec<Arrival>> {
    let mut arrived = Vec::new();
    let first = match until {
        None => inbox.recv().ok(),
        Some(until) => match inbox.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(arrival) => Some(arrival),
            Err(RecvTimeoutError::Timeout) => return Some(arrived),
            Err(RecvTimeoutError::Disconnected) => None,
        },
    };
    arrived.push(first?);
    loop {
        match inbox.try_recv() {
            Ok(arrival) => arrived.push(arrival),
            // Nothing more for now, or every sender gone after what arrived;
            // the next look finds nobody.
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Some(arrived),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_last_order_sender_going_is_said_once() {
        let (orders, results, inbox) = inbox();
        let copy = orders.clone();
        drop(orders);
        assert!(inbox.try_recv().is_err(), "a copy is still held");
        drop(copy);
        assert!(matches!(inbox.try_recv(), Ok(Arrival::Abandoned)));
        assert!(inbox.try_recv().is_err(), "and only once");
        drop(results);
    }

    #[test]
    fn an_order_arrives_as_it_was_sent() {
        let (orders, _results, inbox) = inbox();
        assert!(orders.send(Order::CloseEverything).is_ok());
        let arrived = arrivals(&inbox, None).unwrap_or_default();
        assert!(matches!(
            arrived.as_slice(),
            [Arrival::Order(Order::CloseEverything)]
        ));
    }

    #[test]
    fn a_bound_ends_the_wait_with_nothing() {
        let (_orders, _results, inbox) = inbox();
        let until = Instant::now()
            .checked_add(Duration::from_millis(20))
            .unwrap_or_else(Instant::now);
        let arrived = arrivals(&inbox, Some(until));
        assert!(arrived.is_some_and(|arrived| arrived.is_empty()));
        assert!(Instant::now() >= until, "and not before it");
    }

    #[test]
    fn an_order_to_a_conductor_that_finished_comes_back() {
        let (orders, results, inbox) = inbox();
        drop(inbox);
        drop(results);
        assert_eq!(orders.send(Order::CloseEverything), Err(Finished));
    }
}
