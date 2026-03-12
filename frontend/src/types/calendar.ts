export interface CalendarEvent {
  id: string;
  title: string;
  description: string;
  location: string;
  start_time: string;
  end_time: string;
  all_day: boolean;
  recurrence_rule: string | null;
  attendees: string;
  organizer: string;
  status: string;
  source: string;
  source_uid: string | null;
  meeting_url: string | null;
  color: string | null;
  reminder_minutes: number | null;
  created_at: string;
  updated_at: string;
}

export interface CreateEventRequest {
  title: string;
  description?: string;
  location?: string;
  start_time: string;
  end_time: string;
  all_day?: boolean;
  meeting_url?: string;
  color?: string;
  reminder_minutes?: number;
  attendees?: string;
}

export interface UpdateEventRequest {
  title?: string;
  description?: string;
  location?: string;
  start_time?: string;
  end_time?: string;
  all_day?: boolean;
  meeting_url?: string;
  color?: string;
  reminder_minutes?: number;
  status?: string;
  attendees?: string;
}

export interface CalendarSettings {
  default_view: string;
  week_starts_on: number;
  working_hours_start: string;
  working_hours_end: string;
  time_format: string;
  zoom_link: string;
  teams_link: string;
  meet_link: string;
  caldav_url: string;
  caldav_username: string;
  caldav_enabled: boolean;
}

export interface MeetingTemplate {
  id: number;
  name: string;
  url_template: string;
  icon: string;
  is_default: boolean;
}

export interface CalendarEventsResponse {
  events: CalendarEvent[];
}

export interface MeetingTemplatesResponse {
  templates: MeetingTemplate[];
}
