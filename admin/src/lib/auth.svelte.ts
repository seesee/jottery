// Authentication state for the admin dashboard
import { api } from './api';
import { DashboardAuth } from './dashboardAuth.svelte';

export const auth = new DashboardAuth(api);
