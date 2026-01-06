<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount, onDestroy } from 'svelte';

  export let pdfUrl: string;
  export let _filename: string = 'document.pdf'; // Kept for API compatibility

  let canvasContainer: HTMLDivElement;
  let currentPage = 1;
  let totalPages = 0;
  let pdfDoc: any = null;
  let rendering = false;
  let scale = 1.0;
  let pdfjsLib: any = null;
  let loadingLibrary = true;

  async function loadPdf() {
    try {
      // Lazy load PDF.js library (only when needed)
      if (!pdfjsLib) {
        loadingLibrary = true;
        pdfjsLib = await import('pdfjs-dist');

        // Set worker source to local file (bundled in public directory to avoid CSP issues)
        pdfjsLib.GlobalWorkerOptions.workerSrc = '/pdf.worker.min.mjs';
        loadingLibrary = false;
      }

      // Load the PDF document
      const loadingTask = pdfjsLib.getDocument(pdfUrl);
      pdfDoc = await loadingTask.promise;
      totalPages = pdfDoc.numPages;

      // Render first page
      await renderPage(1);
    } catch (error) {
      console.error('Error loading PDF:', error);
      loadingLibrary = false;
    }
  }

  async function renderPage(pageNum: number) {
    if (rendering || !pdfDoc) return;

    rendering = true;

    try {
      const page = await pdfDoc.getPage(pageNum);

      // Calculate scale based on container width
      const viewport = page.getViewport({ scale: 1.0 });
      const containerWidth = canvasContainer?.clientWidth || 800;
      const actualScale = (containerWidth / viewport.width) * scale;
      const scaledViewport = page.getViewport({ scale: actualScale });

      // Clear container
      canvasContainer.innerHTML = '';

      // Create canvas
      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      canvas.height = scaledViewport.height;
      canvas.width = scaledViewport.width;
      canvas.className = 'mx-auto shadow-lg';

      canvasContainer.appendChild(canvas);

      // Render PDF page
      const renderContext = {
        canvasContext: context,
        viewport: scaledViewport,
      };

      await page.render(renderContext).promise;
      currentPage = pageNum;
    } catch (error) {
      console.error('Error rendering page:', error);
    } finally {
      rendering = false;
    }
  }

  function nextPage() {
    if (currentPage < totalPages) {
      renderPage(currentPage + 1);
    }
  }

  function prevPage() {
    if (currentPage > 1) {
      renderPage(currentPage - 1);
    }
  }

  function zoomIn() {
    scale = Math.min(scale + 0.25, 3.0);
    renderPage(currentPage);
  }

  function zoomOut() {
    scale = Math.max(scale - 0.25, 0.5);
    renderPage(currentPage);
  }

  onMount(() => {
    loadPdf();
  });

  onDestroy(() => {
    if (pdfDoc) {
      pdfDoc.destroy();
    }
  });
</script>

<div class="flex flex-col h-full">
  {#if loadingLibrary}
    <div class="flex items-center justify-center h-full">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
        <p class="text-gray-600 dark:text-gray-400">{$_('pdfViewer.loading')}</p>
      </div>
    </div>
  {:else}
    <!-- PDF Controls -->
    <div class="flex items-center justify-between p-3 bg-gray-100 dark:bg-gray-800 border-b border-gray-300 dark:border-gray-700">
    <div class="flex items-center gap-2">
      <button
        on:click={prevPage}
        disabled={currentPage <= 1}
        class="px-3 py-1 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        title={$_('pdfViewer.previousPage')}
      >
        ←
      </button>
      <span class="text-sm text-gray-700 dark:text-gray-300">
        {$_('pdfViewer.pageOfPages', { values: { current: currentPage, total: totalPages } })}
      </span>
      <button
        on:click={nextPage}
        disabled={currentPage >= totalPages}
        class="px-3 py-1 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        title={$_('pdfViewer.nextPage')}
      >
        →
      </button>
    </div>

    <div class="flex items-center gap-2">
      <button
        on:click={zoomOut}
        disabled={scale <= 0.5}
        class="px-3 py-1 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        title={$_('pdfViewer.zoomOut')}
      >
        −
      </button>
      <span class="text-sm text-gray-700 dark:text-gray-300 min-w-[60px] text-center">
        {Math.round(scale * 100)}%
      </span>
      <button
        on:click={zoomIn}
        disabled={scale >= 3.0}
        class="px-3 py-1 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        title={$_('pdfViewer.zoomIn')}
      >
        +
      </button>
    </div>
  </div>

  <!-- PDF Canvas -->
  <div class="flex-1 overflow-auto bg-gray-200 dark:bg-gray-900 p-4">
    <div bind:this={canvasContainer} class="min-h-full"></div>
  </div>
  {/if}
</div>
