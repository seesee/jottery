<script lang="ts">
  export let tags: string[] = [];
  export let onChange: (tags: string[]) => void = () => {};
  export let placeholder: string = 'Add tags...';

  let inputValue = '';
  let suggestions: string[] = [];
  let showSuggestions = false;
  let selectedSuggestionIndex = -1;

  // All available tags from existing notes (would be passed as prop in real implementation)
  export let availableTags: string[] = [];

  function handleInput() {
    if (inputValue.trim()) {
      // Filter available tags for suggestions
      const query = inputValue.toLowerCase();
      suggestions = availableTags
        .filter(tag => !tags.includes(tag) && tag.toLowerCase().includes(query))
        .slice(0, 5);
      showSuggestions = suggestions.length > 0;
      selectedSuggestionIndex = -1;
    } else {
      showSuggestions = false;
      suggestions = [];
    }
  }

  function addTag(tag: string) {
    if (tag.trim() && !tags.includes(tag.trim())) {
      tags = [...tags, tag.trim()];
      onChange(tags);
      inputValue = '';
      showSuggestions = false;
      suggestions = [];
    }
  }

  function removeTag(index: number) {
    tags = tags.filter((_, i) => i !== index);
    onChange(tags);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (selectedSuggestionIndex >= 0 && suggestions[selectedSuggestionIndex]) {
        addTag(suggestions[selectedSuggestionIndex]);
      } else if (inputValue.trim()) {
        addTag(inputValue);
      }
    } else if (e.key === 'Backspace' && !inputValue && tags.length > 0) {
      removeTag(tags.length - 1);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (showSuggestions) {
        selectedSuggestionIndex = Math.min(selectedSuggestionIndex + 1, suggestions.length - 1);
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (showSuggestions) {
        selectedSuggestionIndex = Math.max(selectedSuggestionIndex - 1, -1);
      }
    } else if (e.key === 'Escape') {
      showSuggestions = false;
      selectedSuggestionIndex = -1;
    }
  }

  function handleBlur() {
    // Delay to allow click on suggestion
    setTimeout(() => {
      showSuggestions = false;
      selectedSuggestionIndex = -1;
    }, 200);
  }
</script>

<div class="relative">
  <div class="flex flex-wrap gap-2 p-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 min-h-[2.5rem]">
    <!-- Existing tags -->
    {#each tags as tag, index}
      <span class="inline-flex items-center gap-1 px-2 py-1 bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 text-sm rounded-md">
        #{tag}
        <button
          on:click={() => removeTag(index)}
          class="hover:text-blue-600 dark:hover:text-blue-400"
          title="Remove tag"
        >
          ×
        </button>
      </span>
    {/each}

    <!-- Input -->
    <input
      type="text"
      bind:value={inputValue}
      on:input={handleInput}
      on:keydown={handleKeyDown}
      on:blur={handleBlur}
      {placeholder}
      class="flex-1 min-w-[120px] outline-none bg-transparent text-sm text-gray-900 dark:text-gray-100"
    />
  </div>

  <!-- Suggestions dropdown -->
  {#if showSuggestions}
    <div class="absolute z-10 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto">
      {#each suggestions as suggestion, index}
        <button
          on:click={() => addTag(suggestion)}
          class="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
        >
          #{suggestion}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Prevent layout shift when suggestions appear */
  .relative {
    position: relative;
  }
</style>
