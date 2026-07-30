# frozen_string_literal: true

require "date"

SEED_PROJECT_IDENTIFIER = "redmine-tui-sandbox"
SEED_PROJECT_NAME = "Redmine TUI Sandbox"

def load_default_data_if_needed
  return if Tracker.exists? && IssueStatus.exists? && IssuePriority.exists?

  if defined?(Redmine::DefaultData::Loader)
    Redmine::DefaultData::Loader.load("en")
  end
end

def active_admin
  User.find_by(login: "admin") || User.active.first || User.anonymous
end

def ensure_user(login:, firstname:, lastname:, mail:)
  user = User.find_or_initialize_by(login: login)
  user.firstname = firstname
  user.lastname = lastname
  user.mail = mail
  user.status = Principal::STATUS_ACTIVE
  user.language = "en" if user.respond_to?(:language=)
  user.mail_notification = "only_my_events" if user.respond_to?(:mail_notification=)

  if user.new_record?
    user.password = "password123"
    user.password_confirmation = "password123"
    user.must_change_passwd = false if user.respond_to?(:must_change_passwd=)
  end

  user.save!
  user
end

def first_named_or_first(model, names)
  names.each do |name|
    item = model.find_by(name: name)
    return item if item
  end

  model.first || raise("No #{model.name} records are available. Default Redmine data was not loaded.")
end

def ensure_role
  Role.find_by(name: "Developer") ||
    Role.givable.first ||
    Role.create!(
      name: "Developer",
      permissions: [
        :view_issues,
        :add_issues,
        :edit_issues,
        :add_issue_notes,
        :log_time,
        :view_time_entries
      ]
    )
end

def ensure_project(trackers)
  project = Project.find_or_initialize_by(identifier: SEED_PROJECT_IDENTIFIER)
  project.name = SEED_PROJECT_NAME
  project.description = "Local test project for redmine-tui development."
  project.is_public = true
  project.status = Project::STATUS_ACTIVE if defined?(Project::STATUS_ACTIVE)

  available_modules = Redmine::AccessControl.available_project_modules.map(&:to_s)
  project.enabled_module_names = %w[issue_tracking time_tracking wiki].select { |name| available_modules.include?(name) }
  project.save!

  project.trackers = trackers
  project.save!
  project
end

def ensure_member(project, user, role)
  member = Member.find_or_initialize_by(project: project, user: user)
  member.role_ids = (member.role_ids + [role.id]).uniq
  member.save!
  member
end

def ensure_version(project)
  version = Version.find_or_initialize_by(project: project, name: "TUI Test v1.0")
  version.description = "Version used by local redmine-tui smoke data."
  version.status = "open"
  version.effective_date = Date.today + 30
  version.save!
  version
end

def ensure_category(project, assigned_to)
  category = IssueCategory.find_or_initialize_by(project: project, name: "TUI")
  category.assigned_to = assigned_to
  category.save!
  category
end

def ensure_activity
  activity = TimeEntryActivity.find_or_initialize_by(name: "Development")
  activity.active = true if activity.respond_to?(:active=)
  activity.is_default = true if TimeEntryActivity.respond_to?(:default) && TimeEntryActivity.default.nil?
  activity.save!
  activity
end

def ensure_issue(project:, subject:, tracker:, status:, priority:, author:, assigned_to:, version:, category:, description:, done_ratio:, due_in_days:, parent: nil)
  issue = Issue.find_or_initialize_by(project_id: project.id, subject: subject)
  issue.tracker = tracker
  issue.status = status
  issue.priority = priority
  issue.author = author
  issue.assigned_to = assigned_to
  issue.fixed_version = version
  issue.category = category
  issue.description = description
  issue.start_date = Date.today - 7
  issue.due_date = Date.today + due_in_days
  issue.done_ratio = done_ratio
  issue.parent_issue_id = parent.id if parent
  issue.save!
  issue.reload
end

def ensure_journal(issue, user, notes)
  issue.reload
  return if issue.journals.where(notes: notes).exists?

  issue.init_journal(user, notes)
  issue.save!
  issue.reload
end

def ensure_time_entry(project:, issue:, user:, activity:, spent_on:, hours:, comments:)
  entry = TimeEntry.find_or_initialize_by(issue: issue, user: user, spent_on: spent_on, comments: comments)
  entry.project = project
  entry.activity = activity
  entry.hours = hours
  entry.save!
  entry
end

load_default_data_if_needed

admin = active_admin
User.current = admin

previous_notified_events = Setting.notified_events
Setting.notified_events = []
at_exit { Setting.notified_events = previous_notified_events }

alice = ensure_user(
  login: "alice.tui",
  firstname: "Alice",
  lastname: "Tui",
  mail: "alice.tui@example.test"
)
bob = ensure_user(
  login: "bob.tui",
  firstname: "Bob",
  lastname: "Tui",
  mail: "bob.tui@example.test"
)

trackers = [
  first_named_or_first(Tracker, ["Bug"]),
  first_named_or_first(Tracker, ["Feature"]),
  first_named_or_first(Tracker, ["Support"])
].uniq
status_new = first_named_or_first(IssueStatus, ["New"])
status_in_progress = first_named_or_first(IssueStatus, ["In Progress", "Assigned"])
priority_normal = IssuePriority.default || first_named_or_first(IssuePriority, ["Normal"])
priority_high = first_named_or_first(IssuePriority, ["High", "Urgent"])
role = ensure_role

project = ensure_project(trackers)
[admin, alice, bob].each { |user| ensure_member(project, user, role) if user && !user.anonymous? }

version = ensure_version(project)
category = ensure_category(project, alice)
activity = ensure_activity

parent_issue = ensure_issue(
  project: project,
  subject: "Prepare Redmine TUI integration test project",
  tracker: trackers.first,
  status: status_in_progress,
  priority: priority_high,
  author: admin,
  assigned_to: alice,
  version: version,
  category: category,
  done_ratio: 40,
  due_in_days: 14,
  description: <<~TEXT.strip
    Parent issue used to verify Redmine TUI issue headers, long descriptions, status display, assignee display, versions, categories, and progress.

    - Contains markdown-like bullet text
    - Has child issues
    - Has journals and time entries
  TEXT
)

child_issue = ensure_issue(
  project: project,
  subject: "Render child issue list with mixed statuses",
  tracker: trackers[1] || trackers.first,
  status: status_new,
  priority: priority_normal,
  author: admin,
  assigned_to: bob,
  version: version,
  category: category,
  done_ratio: 10,
  due_in_days: 7,
  parent: parent_issue,
  description: "Child issue for checking nested issue rendering in redmine-tui."
)

support_issue = ensure_issue(
  project: project,
  subject: "Check journal wrapping and time entry display",
  tracker: trackers[2] || trackers.first,
  status: status_new,
  priority: priority_normal,
  author: alice,
  assigned_to: alice,
  version: version,
  category: category,
  done_ratio: 0,
  due_in_days: 21,
  description: "Standalone issue with multiple comments and logged time."
)

ensure_journal(parent_issue, alice, "Initial local seed comment for parent issue.")
ensure_journal(parent_issue, bob, "Follow-up note with enough text to exercise wrapping in a narrow terminal window.")
ensure_journal(child_issue, bob, "Child issue note for list/detail navigation checks.")
ensure_journal(support_issue, alice, "Support issue journal entry for history rendering checks.")

ensure_time_entry(
  project: project,
  issue: parent_issue,
  user: alice,
  activity: activity,
  spent_on: Date.today - 2,
  hours: 1.5,
  comments: "Reviewed TUI integration flow"
)
ensure_time_entry(
  project: project,
  issue: support_issue,
  user: bob,
  activity: activity,
  spent_on: Date.today - 1,
  hours: 2.0,
  comments: "Checked journal and time entry rendering"
)

puts "Seeded #{SEED_PROJECT_NAME} (#{SEED_PROJECT_IDENTIFIER})"
puts "Users: #{[alice.login, bob.login].join(", ")}"
puts "Issues: #{[parent_issue.id, child_issue.id, support_issue.id].join(", ")}"
