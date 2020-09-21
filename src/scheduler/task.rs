// Copyright (c) 2017 Stefan Lankes, RWTH Aachen University
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![allow(dead_code)]

use alloc;
use alloc::alloc::{alloc, dealloc, Layout};
use alloc::collections::LinkedList;
use alloc::rc::Rc;
use consts::*;
use core::cell::RefCell;
use core::fmt;
use logging::*;

extern "C" {
	fn get_bootstack() -> *mut u8;
}

/// The status of the task - used for scheduling
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TaskStatus {
	TaskInvalid,
	TaskReady,
	TaskRunning,
	TaskBlocked,
	TaskFinished,
	TaskIdle,
}

/// Unique identifier for a task (i.e. `pid`).
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct TaskId(u32);

impl TaskId {
	pub const fn into(self) -> u32 {
		self.0
	}

	pub const fn from(x: u32) -> Self {
		TaskId(x)
	}
}

impl alloc::fmt::Display for TaskId {
	fn fmt(&self, f: &mut fmt::Formatter) -> alloc::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Priority of a task
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct TaskPriority(u8);

impl TaskPriority {
	pub const fn into(self) -> u8 {
		self.0
	}

	pub const fn from(x: u8) -> Self {
		TaskPriority(x)
	}
}

impl alloc::fmt::Display for TaskPriority {
	fn fmt(&self, f: &mut alloc::fmt::Formatter) -> alloc::fmt::Result {
		write!(f, "{}", self.0)
	}
}

pub const REALTIME_PRIORITY: TaskPriority = TaskPriority::from(0);
pub const HIGH_PRIORITY: TaskPriority = TaskPriority::from(0);
pub const NORMAL_PRIORITY: TaskPriority = TaskPriority::from(24);
pub const LOW_PRIORITY: TaskPriority = TaskPriority::from(NO_PRIORITIES as u8 - 1);

#[derive(Copy, Clone)]
#[repr(align(64))]
#[repr(C)]
pub struct Stack {
	buffer: [u8; STACK_SIZE],
}

impl Stack {
	pub const fn new() -> Stack {
		Stack {
			buffer: [0; STACK_SIZE],
		}
	}

	pub fn top(&self) -> usize {
		(&(self.buffer[STACK_SIZE - 16]) as *const _) as usize
	}

	pub fn bottom(&self) -> usize {
		(&(self.buffer[0]) as *const _) as usize
	}
}

pub static mut BOOT_STACK: Stack = Stack::new();

pub struct TaskQueue {
	queue: LinkedList<Rc<RefCell<Task>>>,
}

impl TaskQueue {
	pub fn new() -> TaskQueue {
		TaskQueue {
			queue: Default::default(),
		}
	}

	/// Add a task to the queue
	pub fn push(&mut self, task: Rc<RefCell<Task>>) {
		self.queue.push_back(task);
	}

	/// Pop the task from the queue
	pub fn pop(&mut self) -> Option<Rc<RefCell<Task>>> {
		self.queue.pop_front()
	}

	#[inline(always)]
	pub fn is_empty(&self) -> bool {
		self.queue.is_empty()
	}

	/// Remove a specific task from the priority queue.
	pub fn remove(&mut self, task: Rc<RefCell<Task>>) {
		let mut cursor = self.queue.cursor_front_mut();

		// Loop through all blocked tasks to find it.
		while let Some(node) = cursor.current() {
			if Rc::ptr_eq(&node, &task) {
				// Remove it from the list
				cursor.remove_current();

				break;
			}
		}
	}
}

impl Default for TaskQueue {
	fn default() -> Self {
		Self {
			queue: Default::default(),
		}
	}
}
/// A task control block, which identifies either a process or a thread
#[repr(align(64))]
pub struct Task {
	/// The ID of this context
	pub id: TaskId,
	/// Task Priority
	pub prio: TaskPriority,
	/// Status of a task, e.g. if the task is ready or blocked
	pub status: TaskStatus,
	/// Last stack pointer before a context switch to another task
	pub last_stack_pointer: usize,
	// Stack of the task
	pub stack: *mut Stack,
}

impl Task {
	pub fn new_idle(id: TaskId) -> Task {
		Task {
			id: id,
			prio: LOW_PRIORITY,
			status: TaskStatus::TaskIdle,
			last_stack_pointer: 0,
			stack: unsafe { &mut BOOT_STACK },
		}
	}

	pub fn new(id: TaskId, status: TaskStatus, prio: TaskPriority) -> Task {
		let stack = unsafe { alloc(Layout::new::<Stack>()) as *mut Stack };

		debug!("Allocate stack for task {} at 0x{:x}", id, stack as usize);

		Task {
			id: id,
			prio: prio,
			status: status,
			last_stack_pointer: 0,
			stack: stack,
		}
	}
}

pub trait TaskFrame {
	/// Create the initial stack frame for a new task
	fn create_stack_frame(&mut self, func: extern "C" fn());
}

impl Drop for Task {
	fn drop(&mut self) {
		if unsafe { self.stack != &mut BOOT_STACK } {
			debug!(
				"Deallocate stack of task {} (stack at 0x{:x})",
				self.id, self.stack as usize
			);

			// deallocate stack
			unsafe {
				dealloc(self.stack as *mut u8, Layout::new::<Stack>());
			}
		}
	}
}
