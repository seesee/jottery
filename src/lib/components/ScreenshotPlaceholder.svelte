<script lang="ts">
  export let width: number = 1200;
  export let height: number = 800;
  export let description: string = 'Screenshot placeholder';
  export let icon: string = '🖼️';

  // Calculate aspect ratio for responsive sizing
  const aspectRatio = (height / width) * 100;
</script>

<div class="screenshot-placeholder w-full" style="padding-bottom: {aspectRatio}%;">
  <div class="placeholder-content">
    <div class="placeholder-icon text-4xl md:text-6xl mb-4">
      {icon}
    </div>
    <div class="placeholder-text">
      <p class="text-sm md:text-base font-medium mb-2">{description}</p>
      <p class="text-xs md:text-sm text-gray-500 dark:text-gray-400">
        {width} × {height}px
      </p>
    </div>
  </div>
</div>

<style>
  .screenshot-placeholder {
    position: relative;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    background-size: 200% 200%;
    animation: gradient 15s ease infinite;
    border-radius: 0.75rem;
    overflow: hidden;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  .placeholder-content {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    text-align: center;
    color: white;
    background: rgba(0, 0, 0, 0.2);
    backdrop-filter: blur(8px);
  }

  .placeholder-text {
    max-width: 80%;
  }

  @keyframes gradient {
    0% {
      background-position: 0% 50%;
    }
    50% {
      background-position: 100% 50%;
    }
    100% {
      background-position: 0% 50%;
    }
  }

  /* Dark mode adjustments */
  :global(.dark) .screenshot-placeholder {
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.3), 0 10px 10px -5px rgba(0, 0, 0, 0.2);
  }
</style>
