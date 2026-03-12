use crate::error::Result;
use crate::models::meeting::{
    AttendanceRecord, CreateMeetingRequest, MeetingAttendanceReport, OnlineMeeting,
    UpdateMeetingRequest,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_meetings(
    client: &GraphClient,
    pagination: &PaginationOpts,
) -> Result<Vec<OnlineMeeting>> {
    client
        .get_paged(&endpoints::my_online_meetings(), &[], pagination)
        .await
}

pub async fn get_meeting(client: &GraphClient, meeting_id: &str) -> Result<OnlineMeeting> {
    client
        .get(&endpoints::online_meeting(meeting_id), &[])
        .await
}

pub async fn create_meeting(
    client: &GraphClient,
    req: &CreateMeetingRequest,
) -> Result<OnlineMeeting> {
    client.post(&endpoints::my_online_meetings(), req).await
}

pub async fn update_meeting(
    client: &GraphClient,
    meeting_id: &str,
    req: &UpdateMeetingRequest,
) -> Result<OnlineMeeting> {
    client
        .patch(&endpoints::online_meeting(meeting_id), req)
        .await
}

pub async fn delete_meeting(client: &GraphClient, meeting_id: &str) -> Result<()> {
    client.delete(&endpoints::online_meeting(meeting_id)).await
}

pub async fn list_attendance_reports(
    client: &GraphClient,
    meeting_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<MeetingAttendanceReport>> {
    client
        .get_paged(
            &endpoints::meeting_attendance_reports(meeting_id),
            &[],
            pagination,
        )
        .await
}

#[allow(dead_code)]
pub async fn get_attendance_report(
    client: &GraphClient,
    meeting_id: &str,
    report_id: &str,
) -> Result<MeetingAttendanceReport> {
    client
        .get(
            &endpoints::meeting_attendance_report(meeting_id, report_id),
            &[],
        )
        .await
}

pub async fn list_attendance_records(
    client: &GraphClient,
    meeting_id: &str,
    report_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<AttendanceRecord>> {
    client
        .get_paged(
            &endpoints::attendance_records(meeting_id, report_id),
            &[],
            pagination,
        )
        .await
}
