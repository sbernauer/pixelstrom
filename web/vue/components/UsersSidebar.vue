<script setup>
import { ref, onMounted } from 'vue';

const props = defineProps({
  currentUser: String,
  upcomingUsers: Array,
});
</script>

<template>
  <div id="users-sidebar">
    <div id="currently-painting">
      <h2>Currently painting</h2>
      <div>
        {{ props.currentUser }}
      </div>
    </div>
    <div id="upcoming">
      <h3>Upcoming painters</h3>
      <div class="upcoming-list">
        <div v-for="(user, index) in props.upcomingUsers" :key="index">
          {{ user }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
#users-sidebar {
  flex: initial;
  background-color: #1e1e1e;
  color: white;
  display: flex;
  flex-direction: column;
  padding: 16px;
  box-shadow: -2px 0 5px rgba(0, 0, 0, 0.5);
}
</style>
