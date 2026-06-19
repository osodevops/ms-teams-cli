use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::meeting::{CreateMeetingRequest, UpdateMeetingRequest};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    /// List online meetings created by you
    List,
    /// Get a meeting by ID
    Get {
        /// Meeting ID
        meeting_id: String,
    },
    /// Create an online meeting
    Create {
        /// Meeting subject
        #[arg(long)]
        subject: String,
        /// Start time (ISO 8601)
        #[arg(long)]
        start: Option<String>,
        /// End time (ISO 8601)
        #[arg(long)]
        end: Option<String>,
        /// Allowed presenters (everyone, organization, roleIsPresenter, organizer)
        #[arg(long)]
        allowed_presenters: Option<String>,
    },
    /// Update a meeting
    Update {
        /// Meeting ID
        meeting_id: String,
        /// New subject
        #[arg(long)]
        subject: Option<String>,
        /// New start time (ISO 8601)
        #[arg(long)]
        start: Option<String>,
        /// New end time (ISO 8601)
        #[arg(long)]
        end: Option<String>,
        /// Allowed presenters
        #[arg(long)]
        allowed_presenters: Option<String>,
    },
    /// Delete a meeting
    Delete {
        /// Meeting ID
        meeting_id: String,
    },
    /// Get the join URL for a meeting
    JoinUrl {
        /// Meeting ID
        meeting_id: String,
    },
    /// List attendance reports for a meeting
    Attendance {
        /// Meeting ID
        meeting_id: String,
        /// Report ID (if provided, lists attendance records for this report)
        #[arg(long)]
        report_id: Option<String>,
    },
}

pub async fn run(
    cmd: MeetingCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile).await?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        MeetingCommand::List => {
            let start = Instant::now();
            let meetings = api::meetings::list_meetings(&client, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Subject", "Start", "End", "Join URL"];
                let rows: Vec<Vec<String>> = meetings
                    .iter()
                    .map(|m| {
                        vec![
                            m.id.clone().unwrap_or_default(),
                            m.subject.clone().unwrap_or_default(),
                            m.start_date_time.clone().unwrap_or_default(),
                            m.end_date_time.clone().unwrap_or_default(),
                            m.join_web_url.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &meetings, start);
            }
            Ok(())
        }

        MeetingCommand::Get { meeting_id } => {
            let start = Instant::now();
            let meeting = api::meetings::get_meeting(&client, &meeting_id).await?;
            output::print_success(format, &meeting, start);
            Ok(())
        }

        MeetingCommand::Create {
            subject,
            start,
            end,
            allowed_presenters,
        } => {
            let start_time = Instant::now();
            let req = CreateMeetingRequest {
                subject,
                start_date_time: start,
                end_date_time: end,
                participants: None,
                lobby_bypass_settings: None,
                allowed_presenters,
            };
            let meeting = api::meetings::create_meeting(&client, &req).await?;
            output::print_success(format, &meeting, start_time);
            Ok(())
        }

        MeetingCommand::Update {
            meeting_id,
            subject,
            start,
            end,
            allowed_presenters,
        } => {
            let start_time = Instant::now();
            let req = UpdateMeetingRequest {
                subject,
                start_date_time: start,
                end_date_time: end,
                allowed_presenters,
            };
            let meeting = api::meetings::update_meeting(&client, &meeting_id, &req).await?;
            output::print_success(format, &meeting, start_time);
            Ok(())
        }

        MeetingCommand::Delete { meeting_id } => {
            let start = Instant::now();
            api::meetings::delete_meeting(&client, &meeting_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        MeetingCommand::JoinUrl { meeting_id } => {
            let start = Instant::now();
            let meeting = api::meetings::get_meeting(&client, &meeting_id).await?;
            let url = meeting.join_web_url.unwrap_or_default();
            if format == OutputFormat::Plain {
                println!("{}", url);
            } else {
                let result = serde_json::json!({"joinWebUrl": url});
                output::print_success(format, &result, start);
            }
            Ok(())
        }

        MeetingCommand::Attendance {
            meeting_id,
            report_id,
        } => {
            let start = Instant::now();
            if let Some(rid) = report_id {
                let records =
                    api::meetings::list_attendance_records(&client, &meeting_id, &rid, pagination)
                        .await?;
                if format == OutputFormat::Human {
                    let headers = vec!["ID", "Email", "Role", "Attendance (s)"];
                    let rows: Vec<Vec<String>> = records
                        .iter()
                        .map(|r| {
                            vec![
                                r.id.clone().unwrap_or_default(),
                                r.email_address.clone().unwrap_or_default(),
                                r.role.clone().unwrap_or_default(),
                                r.total_attendance_in_seconds
                                    .map(|s| s.to_string())
                                    .unwrap_or_default(),
                            ]
                        })
                        .collect();
                    output::table::print_table(headers, rows);
                } else {
                    output::print_success_list(format, &records, start);
                }
            } else {
                let reports =
                    api::meetings::list_attendance_reports(&client, &meeting_id, pagination)
                        .await?;
                if format == OutputFormat::Human {
                    let headers = vec!["Report ID", "Participants", "Start", "End"];
                    let rows: Vec<Vec<String>> = reports
                        .iter()
                        .map(|r| {
                            vec![
                                r.id.clone().unwrap_or_default(),
                                r.total_participant_count
                                    .map(|c| c.to_string())
                                    .unwrap_or_default(),
                                r.meeting_start_date_time.clone().unwrap_or_default(),
                                r.meeting_end_date_time.clone().unwrap_or_default(),
                            ]
                        })
                        .collect();
                    output::table::print_table(headers, rows);
                } else {
                    output::print_success_list(format, &reports, start);
                }
            }
            Ok(())
        }
    }
}
