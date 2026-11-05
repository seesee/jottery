// What the server records, and how to say it. An action added to a newer server has
// no entry here, so the page shows the name it was given.
export const AUDIT_ACTIONS: Record<string, string> = {
  'user.approve': 'audit.actions.userApprove',
  'user.deactivate': 'audit.actions.userDeactivate',
  'user.activate': 'audit.actions.userActivate',
  'user.delete': 'audit.actions.userDelete',
  'user.password_reset': 'audit.actions.userPasswordReset',
  'user.admin_grant': 'audit.actions.userAdminGrant',
  'user.admin_revoke': 'audit.actions.userAdminRevoke',
  'user.settings_update': 'audit.actions.userSettingsUpdate',
  'device.revoke': 'audit.actions.deviceRevoke',
  'device.delete': 'audit.actions.deviceDelete',
  'device.rename': 'audit.actions.deviceRename',
  'admin.password_change': 'audit.actions.adminPasswordChange',
};
