use core::cmp::{max, min};
use core::mem;
use core::ptr::null_mut;
use core::time::Duration;
use libc::{gettimeofday, timeval};
use mach_sys::mach_time::{mach_continuous_time, mach_timebase_info, mach_timebase_info_data_t};
use protocol::{AbsoluteTime, Timeout};
use std::collections::LinkedList;

#[derive(Default)]
pub struct Clock {
    /// Sorted absolute timeouts list.
    abs: LinkedList<TimeoutUser>,
    /// Sorted relative timeouts list.
    rel: LinkedList<TimeoutUser>,
    current_time: Timeout,
    monotonic_time: Timeout,
    // struct _KUSER_SHARED_DATA *user_shared_data = NULL;
    // static const timeout_t user_shared_data_timeout = 16 * 10000;
}

static mut MACH_TIMEBASE: mach_timebase_info_data_t = unsafe { mem::zeroed() };

impl Clock {
    pub fn set_current_time(&mut self) {
        let mut now: timeval = unsafe { mem::zeroed() };
        unsafe {
            gettimeofday(&mut now, null_mut());
        }
        self.current_time = Timeout(
            now.tv_sec * Timeout::TICKS_PER_SECOND.0
                + now.tv_usec as i64 * 10
                + Timeout::TICKS_1601_TO_1970.0,
        );
        self.monotonic_time = Clock::monotonic_counter();
        //TODO: if (user_shared_data) set_user_shared_data_time();
    }

    /// Return a monotonic time counter
    pub fn monotonic_counter() -> Timeout {
        #[cfg(target_os = "macos")]
        unsafe {
            if MACH_TIMEBASE.denom == 0 {
                mach_timebase_info(&raw mut MACH_TIMEBASE);
            }
            let time = mach_continuous_time() * MACH_TIMEBASE.numer as u64
                / MACH_TIMEBASE.denom as u64
                / 100;
            Timeout(time as i64)
        }
    }

    pub fn add(&mut self, when: Timeout, callback: fn()) -> TimeoutUser {
        let mut user = TimeoutUser {
            entry: 0,
            when: self.timeout_to_abstime(when),
            callback,
        };
        let mut cursor;
        if user.when.0 > 0 {
            cursor = self.abs.cursor_back_mut();
            while let Some(entry) = cursor.current()
                && entry.when.0 < user.when.0
            {
                cursor.move_next();
            }
        } else {
            cursor = self.rel.cursor_back_mut();
            while let Some(entry) = cursor.current()
                && entry.when.0 > user.when.0
            {
                cursor.move_next();
            }
        }
        user.entry = cursor.index().unwrap_or(0) as isize;
        cursor.insert_before(user);
        user
    }

    pub fn remove(&mut self, user: TimeoutUser) {
        if user.entry > 0 {
            self.abs.remove(user.entry as usize);
        } else if user.entry < 0 {
            self.rel.remove(-user.entry as usize);
        }
    }

    /// Process pending timeouts and return the time until the next timeout, if any.
    pub fn next_timeout(&mut self) -> Option<Duration> {
        //TODO: timeout_t ret = user_shared_data ? user_shared_data_timeout : -1;
        if self.abs.is_empty() && self.rel.is_empty() {
            return None;
        }

        let mut expired = Vec::with_capacity(self.abs.len() + self.rel.len());

        // Remove all expired timers from the list
        let mut cursor = self.abs.cursor_back_mut();
        while let Some(entry) = cursor.current()
            && entry.when.0 <= self.current_time.0
        {
            expired.push(entry.callback);
            cursor.remove_current();
        }
        cursor = self.rel.cursor_back_mut();
        while let Some(entry) = cursor.current()
            && -entry.when.0 <= self.monotonic_time.0
        {
            expired.push(entry.callback);
            cursor.remove_current();
        }

        // Execute all callbacks
        for callback in expired {
            callback();
        }

        // Return the next timeout, if any
        let mut ret = i64::MAX;
        if let Some(entry) = self.abs.front() {
            let diff = max(0, entry.when.0 - self.current_time.0);
            ret = min(ret, diff);
        }
        if let Some(entry) = self.rel.front() {
            let diff = max(0, -entry.when.0 - self.monotonic_time.0);
            ret = min(ret, diff);
        }

        if ret == i64::MAX {
            return None;
        }
        if ret == 0 {
            return Some(Duration::from_nanos(100));
        }

        Some(Duration::from_nanos_u128(ret as u128 * 100))
    }

    pub fn timeout_to_abstime(&self, timeout: Timeout) -> AbsoluteTime {
        if timeout.0 > 0 {
            AbsoluteTime(timeout.0)
        } else {
            AbsoluteTime(timeout.0 - self.monotonic_time.0)
        }
    }

    pub fn abstime_to_timeout(&self, abstime: AbsoluteTime) -> Timeout {
        if abstime.0 > 0 {
            Timeout(abstime.0)
        } else {
            if -abstime.0 < self.monotonic_time.0 {
                Timeout(0)
            } else {
                Timeout(abstime.0 + self.monotonic_time.0)
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct TimeoutUser {
    /// Entry in sorted timeout list.
    entry: isize,
    /// Timeout expiry
    when: AbsoluteTime,
    /// Callback function.
    callback: fn(),
}

/*
static void set_user_shared_data_time(void)
{
    timeout_t tick_count = monotonic_time / 10000;
    static timeout_t last_timezone_update, last_timezone_bias = 65535, adjusted_timezone_bias;
    static int current_year = -1;
    timeout_t timezone_bias;
    struct tm *tm, tm1, tm2;
    time_t now;

    if (monotonic_time - last_timezone_update > TICKS_PER_SEC)
    {
        now = time( NULL );
        tm = gmtime( &now );
        timezone_bias = mktime( tm ) - now;
        tm = localtime( &now );
        if (current_year != tm->tm_year || last_timezone_bias != timezone_bias)
        {
            current_year = tm->tm_year;
            last_timezone_bias = adjusted_timezone_bias = timezone_bias;
            if (tm->tm_isdst)
            {
                tm1 = tm2 = *tm;
                tm1.tm_isdst = 0;
                tm2.tm_isdst = 1;
                adjusted_timezone_bias += mktime(&tm1) < mktime(&tm2) ? 3600 : -3600;
            }
        }
        timezone_bias = adjusted_timezone_bias;
        timezone_bias *= TICKS_PER_SEC;

        atomic_store_long(&user_shared_data->TimeZoneBias.High2Time, timezone_bias >> 32);
        atomic_store_ulong(&user_shared_data->TimeZoneBias.LowPart, timezone_bias);
        atomic_store_long(&user_shared_data->TimeZoneBias.High1Time, timezone_bias >> 32);

        last_timezone_update = monotonic_time;
    }

    atomic_store_long(&user_shared_data->SystemTime.High2Time, current_time >> 32);
    atomic_store_ulong(&user_shared_data->SystemTime.LowPart, current_time);
    atomic_store_long(&user_shared_data->SystemTime.High1Time, current_time >> 32);

    atomic_store_long(&user_shared_data->InterruptTime.High2Time, monotonic_time >> 32);
    atomic_store_ulong(&user_shared_data->InterruptTime.LowPart, monotonic_time);
    atomic_store_long(&user_shared_data->InterruptTime.High1Time, monotonic_time >> 32);

    atomic_store_long(&user_shared_data->TickCount.High2Time, tick_count >> 32);
    atomic_store_ulong(&user_shared_data->TickCount.LowPart, tick_count);
    atomic_store_long(&user_shared_data->TickCount.High1Time, tick_count >> 32);
    atomic_store_ulong(&user_shared_data->TickCountLowDeprecated, tick_count);
}

/* return a text description of a timeout for debugging purposes */
const char *get_timeout_str( timeout_t timeout )
{
    static char buffer[64];
    long secs, nsecs;

    if (!timeout) return "0";
    if (timeout == TIMEOUT_INFINITE) return "infinite";

    if (timeout < 0)  /* relative */
    {
        secs = -timeout / TICKS_PER_SEC;
        nsecs = -timeout % TICKS_PER_SEC;
        snprintf( buffer, sizeof(buffer), "+%ld.%07ld", secs, nsecs );
    }
    else  /* absolute */
    {
        secs = (timeout - current_time) / TICKS_PER_SEC;
        nsecs = (timeout - current_time) % TICKS_PER_SEC;
        if (nsecs < 0)
        {
            nsecs += TICKS_PER_SEC;
            secs--;
        }
        if (secs >= 0)
            snprintf( buffer, sizeof(buffer), "%x%08x (+%ld.%07ld)",
                      (unsigned int)(timeout >> 32), (unsigned int)timeout, secs, nsecs );
        else
            snprintf( buffer, sizeof(buffer), "%x%08x (-%ld.%07ld)",
                      (unsigned int)(timeout >> 32), (unsigned int)timeout,
                      -(secs + 1), TICKS_PER_SEC - nsecs );
    }
    return buffer;
}
 */
