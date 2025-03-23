<script setup>

import UserDetails from "@/components/UserDetails.vue";
const props = defineProps({
  currentUser: String,
  users: Array
});
</script>

<template>
  <div id="users-sidebar">
    <h2>Users</h2>
    <div class="user-table">
      <div>&nbsp;</div>
      <div>Name</div>
      <div>Pixels/sec</div>
      <div>Response Ms</div>
      <UserDetails v-for="user in props.users" :is-painting-user="currentUser === user.username" :user="user"/>
    </div>
  </div>
</template>

<style scoped>
#users-sidebar {
  background-color: #1e1e1e;
  color: white;
  display: flex;
  flex-direction: column;
  padding: 16px;
  box-shadow: -2px 0 5px rgba(0, 0, 0, 0.5);
}

.user-table {
	display: grid;
	grid-template-rows: repeat(auto-fit, auto);
	grid-template-columns: 18px auto auto auto;
	gap: 8px 8px;
}
</style>
