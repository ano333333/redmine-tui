SET NAMES utf8mb4;
SET FOREIGN_KEY_CHECKS = 0;

DELETE FROM journal_details;
DELETE FROM journals;
DELETE FROM time_entries;
DELETE FROM issues;
DELETE FROM issue_categories;
DELETE FROM versions;
DELETE FROM enabled_modules;
DELETE FROM projects_trackers;
DELETE FROM projects;
DELETE FROM trackers;
DELETE FROM issue_statuses;
DELETE FROM enumerations;
DELETE FROM email_addresses WHERE address LIKE 'redmine-tui-%@example.test';
DELETE FROM user_preferences WHERE user_id >= 1000;
DELETE FROM users WHERE id >= 1000 AND login LIKE 'redmine-tui-%';

ALTER TABLE journal_details AUTO_INCREMENT = 1;
ALTER TABLE journals AUTO_INCREMENT = 1;
ALTER TABLE time_entries AUTO_INCREMENT = 1;
ALTER TABLE issues AUTO_INCREMENT = 1;
ALTER TABLE issue_categories AUTO_INCREMENT = 1;
ALTER TABLE versions AUTO_INCREMENT = 1;
ALTER TABLE enabled_modules AUTO_INCREMENT = 1;
ALTER TABLE projects AUTO_INCREMENT = 1;
ALTER TABLE trackers AUTO_INCREMENT = 1;
ALTER TABLE issue_statuses AUTO_INCREMENT = 1;
ALTER TABLE enumerations AUTO_INCREMENT = 1;

SET FOREIGN_KEY_CHECKS = 1;
