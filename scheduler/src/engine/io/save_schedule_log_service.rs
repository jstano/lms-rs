//! Port of `com.unifocus.watson.server.scheduler.engine.io.SaveScheduleLogService`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! SaveScheduleLogService.java`. Java writes the log to a temp file as gzipped XML
//! (`ScheduleLogWriter`, `java.util.zip.GZIPOutputStream`) then reads the whole file back into a
//! byte array before deleting it — a round-trip through the filesystem this crate has no
//! equivalent for. `ScheduleLogWriter` itself (the actual XML emission) is out of this wave's
//! scope, same "deferred external subsystem" treatment as everywhere else in this crate
//! (`PARITY_AUDIT.md` findings 10/16/21/37) — [`ScheduleLogWriterPort`] wraps it, writing straight
//! into an in-memory buffer instead of a temp file/gzip stream. What *is* real: which schedule
//! logs get written, and in what order (`ScheduleModel::job_schedule_logs`'s sort, then each job's
//! filters in insertion order) — that traversal is ported for real, not stubbed.
//!
//! `DateTimeServices.currentDateTime()` becomes `joda_rs::LocalDateTime::now()` directly, not a
//! port — cheap to call faithfully (same "port it for real, it's cheap" treatment as
//! `common::java_random::JavaRandom`, `PARITY_AUDIT.md` finding 23), and nothing needs to control
//! or observe it in a test.

use crate::engine::io::ports::{ReportLibrary, ReportLibraryPort};
use crate::engine::model::logging::schedule_log::ScheduleLog;
use crate::engine::model::schedule_model::ScheduleModel;
use joda_rs::LocalDateTime;

const GENERATE_SCHEDULES_LOG: &str = "Generate Schedules Log";

/// `ScheduleLogWriter`, narrowed to the three calls `SaveScheduleLogService.saveToTempFile`
/// makes — the actual XML/gzip encoding is out of this wave's scope (see module doc).
pub trait ScheduleLogWriterPort {
    /// `outputHeader(PrintWriter)`.
    fn output_header(&self, buf: &mut Vec<u8>);

    /// `outputScheduleLog(ScheduleLog, PrintWriter)`.
    fn output_schedule_log(&self, schedule_log: &ScheduleLog, buf: &mut Vec<u8>);

    /// `outputFooter(PrintWriter)`.
    fn output_footer(&self, buf: &mut Vec<u8>);
}

/// `SaveScheduleLogService`.
pub struct SaveScheduleLogService<'a> {
    report_library: &'a dyn ReportLibraryPort,
    schedule_log_writer: &'a dyn ScheduleLogWriterPort,
}

impl<'a> SaveScheduleLogService<'a> {
    pub fn new(
        report_library: &'a dyn ReportLibraryPort,
        schedule_log_writer: &'a dyn ScheduleLogWriterPort,
    ) -> Self {
        Self {
            report_library,
            schedule_log_writer,
        }
    }

    /// `saveScheduleLogs(ScheduleModel)`.
    pub fn save_schedule_logs(&self, schedule_model: &ScheduleModel) {
        self.report_library
            .save(&self.create_report_content(schedule_model));
    }

    /// `createReportContent(ScheduleModel)`.
    fn create_report_content(&self, schedule_model: &ScheduleModel) -> ReportLibrary {
        let report_data = self.write_schedule_logs(schedule_model);

        self.create_report_library_entry(schedule_model, report_data)
    }

    /// `saveToTempFile(List<JobScheduleLog>, File)` + `readFromTempFile(File)`, collapsed into
    /// one in-memory pass (see module doc).
    fn write_schedule_logs(&self, schedule_model: &ScheduleModel) -> Vec<u8> {
        let mut buf = Vec::new();

        self.schedule_log_writer.output_header(&mut buf);

        for job_schedule_log in schedule_model.job_schedule_logs() {
            for &employee_filter_key in job_schedule_log.employee_filter_keys() {
                if let Some(schedule_log) =
                    job_schedule_log.schedule_log_for_key(employee_filter_key)
                {
                    self.schedule_log_writer
                        .output_schedule_log(schedule_log, &mut buf);
                }
            }
        }

        self.schedule_log_writer.output_footer(&mut buf);

        buf
    }

    /// `createReportLibraryEntry(ScheduleModel, byte[])`.
    fn create_report_library_entry(
        &self,
        schedule_model: &ScheduleModel,
        report_data: Vec<u8>,
    ) -> ReportLibrary {
        ReportLibrary::new(
            schedule_model.property().id(),
            GENERATE_SCHEDULES_LOG,
            LocalDateTime::now(),
            "xml",
            report_data.clone(),
            report_data,
            schedule_model.date_range().start_date(),
            schedule_model.date_range().end_date(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::process::variable::filters::{
        JobLevelEmployeeFilter, SalariedHomeJobOnlyEmployeeFilter,
    };
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;
    use std::cell::RefCell;

    #[derive(Default)]
    struct SpyWriter {
        header_written: RefCell<bool>,
        schedule_logs_written: RefCell<usize>,
        footer_written: RefCell<bool>,
    }

    impl ScheduleLogWriterPort for SpyWriter {
        fn output_header(&self, buf: &mut Vec<u8>) {
            *self.header_written.borrow_mut() = true;
            buf.extend_from_slice(b"<header>");
        }

        fn output_schedule_log(&self, _schedule_log: &ScheduleLog, buf: &mut Vec<u8>) {
            *self.schedule_logs_written.borrow_mut() += 1;
            buf.extend_from_slice(b"<log/>");
        }

        fn output_footer(&self, buf: &mut Vec<u8>) {
            *self.footer_written.borrow_mut() = true;
            buf.extend_from_slice(b"</header>");
        }
    }

    #[derive(Default)]
    struct SpyReportLibraryDAO {
        saved: RefCell<Option<ReportLibrary>>,
    }

    impl ReportLibraryPort for SpyReportLibraryDAO {
        fn save(&self, report_library: &ReportLibrary) {
            *self.saved.borrow_mut() = Some(report_library.clone());
        }
    }

    #[test]
    fn writes_every_job_schedule_log_for_every_filter_it_has_and_saves_the_report() {
        let date_range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(7, date_range);
        let mut schedule_model = ScheduleModel::new(property, date_range);

        schedule_model
            .job_schedule_log(1)
            .schedule_log(Box::new(SalariedHomeJobOnlyEmployeeFilter));
        schedule_model
            .job_schedule_log(1)
            .schedule_log(Box::new(JobLevelEmployeeFilter::new(2)));
        schedule_model
            .job_schedule_log(2)
            .schedule_log(Box::new(SalariedHomeJobOnlyEmployeeFilter));

        let writer = SpyWriter::default();
        let dao = SpyReportLibraryDAO::default();
        let service = SaveScheduleLogService::new(&dao, &writer);

        service.save_schedule_logs(&schedule_model);

        assert!(*writer.header_written.borrow());
        assert!(*writer.footer_written.borrow());
        assert_eq!(*writer.schedule_logs_written.borrow(), 3);

        let saved = dao.saved.borrow();
        let saved = saved.as_ref().expect("report_library should be saved");
        assert_eq!(saved.property_id(), 7);
        assert_eq!(saved.report_name(), GENERATE_SCHEDULES_LOG);
        assert_eq!(saved.report_type(), "xml");
        assert_eq!(saved.start_date(), LocalDate::of(2024, 1, 1));
        assert_eq!(saved.end_date(), LocalDate::of(2024, 1, 7));
        assert_eq!(saved.report_data(), saved.html_report_data());
        assert_eq!(saved.report_data(), b"<header><log/><log/><log/></header>");
    }
}
