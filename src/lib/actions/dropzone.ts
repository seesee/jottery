/**
 * Dropzone Svelte Action
 *
 * A reusable action for drag-and-drop file handling.
 * Supports file type filtering, disabled state, and proper
 * handling of nested drag events.
 *
 * Usage:
 *   <div use:dropzone={{ onDrop, onDragStateChange }}>
 *     Drop files here
 *   </div>
 */

export interface DropzoneOptions {
  /** Callback when files are dropped */
  onDrop?: (files: FileList) => void;

  /** Callback when drag state changes (entering/leaving the drop zone) */
  onDragStateChange?: (isDragging: boolean) => void;

  /** Whether the dropzone is disabled */
  disabled?: boolean;

  /** Only accept specific MIME types (e.g., ['image/*', 'application/pdf']) */
  accept?: string[];
}

/**
 * Svelte action for drag-and-drop file handling
 */
export function dropzone(node: HTMLElement, options: DropzoneOptions = {}) {
  let { onDrop, onDragStateChange, disabled = false, accept } = options;

  // Counter to track nested drag events
  // This is necessary because dragenter/dragleave fire for child elements too
  let dragCounter = 0;

  function handleDragEnter(e: DragEvent) {
    if (disabled) return;

    e.preventDefault();
    e.stopPropagation();

    dragCounter++;

    // Only trigger on first enter (entering the zone)
    if (dragCounter === 1 && e.dataTransfer?.types.includes('Files')) {
      onDragStateChange?.(true);
    }
  }

  function handleDragLeave(e: DragEvent) {
    if (disabled) return;

    e.preventDefault();
    e.stopPropagation();

    dragCounter--;

    // Only trigger when fully leaving the zone
    if (dragCounter === 0) {
      onDragStateChange?.(false);
    }
  }

  function handleDragOver(e: DragEvent) {
    if (disabled) return;

    e.preventDefault();
    e.stopPropagation();

    // Set the drop effect to copy
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'copy';
    }
  }

  function handleDrop(e: DragEvent) {
    if (disabled) return;

    e.preventDefault();
    e.stopPropagation();

    // Reset drag state
    dragCounter = 0;
    onDragStateChange?.(false);

    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;

    // Filter files by accepted types if specified
    if (accept && accept.length > 0) {
      const filteredFiles = filterFilesByType(files, accept);
      if (filteredFiles.length > 0) {
        onDrop?.(createFileList(filteredFiles));
      }
    } else {
      onDrop?.(files);
    }
  }

  // Add event listeners
  node.addEventListener('dragenter', handleDragEnter);
  node.addEventListener('dragleave', handleDragLeave);
  node.addEventListener('dragover', handleDragOver);
  node.addEventListener('drop', handleDrop);

  return {
    update(newOptions: DropzoneOptions) {
      onDrop = newOptions.onDrop;
      onDragStateChange = newOptions.onDragStateChange;
      disabled = newOptions.disabled ?? false;
      accept = newOptions.accept;
    },

    destroy() {
      node.removeEventListener('dragenter', handleDragEnter);
      node.removeEventListener('dragleave', handleDragLeave);
      node.removeEventListener('dragover', handleDragOver);
      node.removeEventListener('drop', handleDrop);
    },
  };
}

/**
 * Filter files by accepted MIME types
 */
function filterFilesByType(files: FileList, accept: string[]): File[] {
  const result: File[] = [];

  for (let i = 0; i < files.length; i++) {
    const file = files[i];
    if (matchesAccept(file.type, accept)) {
      result.push(file);
    }
  }

  return result;
}

/**
 * Check if a MIME type matches any of the accepted patterns
 */
function matchesAccept(mimeType: string, accept: string[]): boolean {
  for (const pattern of accept) {
    if (pattern === '*' || pattern === '*/*') {
      return true;
    }

    if (pattern.endsWith('/*')) {
      // Wildcard type (e.g., 'image/*')
      const typePrefix = pattern.slice(0, -2);
      if (mimeType.startsWith(typePrefix + '/')) {
        return true;
      }
    } else if (pattern === mimeType) {
      // Exact match
      return true;
    }
  }

  return false;
}

/**
 * Create a FileList-like object from an array of Files
 */
function createFileList(files: File[]): FileList {
  const dataTransfer = new DataTransfer();
  for (const file of files) {
    dataTransfer.items.add(file);
  }
  return dataTransfer.files;
}

export default dropzone;
