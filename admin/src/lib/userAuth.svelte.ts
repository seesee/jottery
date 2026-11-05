// Authentication state for the user portal
import { userApi } from './userApi';
import { DashboardAuth } from './dashboardAuth.svelte';

export const userAuth = new DashboardAuth(userApi);
