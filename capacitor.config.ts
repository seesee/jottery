import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'io.jottery.app',
  appName: 'Jottery',
  webDir: 'dist',
  ios: {
    // Prevent WKWebView's built-in text zoom so we can control
    // font scaling ourselves via the Dynamic Type probe.
    preferredContentMode: 'mobile',
  },
  plugins: {
    SplashScreen: {
      launchAutoHide: true,
      androidScaleType: 'CENTER_CROP',
      showSpinner: false,
    },
    Keyboard: {
      resize: 'body',
      resizeOnFullScreen: true,
    },
    StatusBar: {
      overlaysWebView: true,
    },
  },
};

export default config;
