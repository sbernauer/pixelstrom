<script>

import UserDetails from "@/components/user/UserDetails.vue";
import UnfollowUsersButton from "@/components/user/UnfollowUsersButton.vue";

export default {
  name: 'UserSidebar',
  components: { UserDetails, UnfollowUsersButton },
  props: {
    users: {
      type: Array,
      required: true,
    },
    currentPaintingUser:{
      type: String,
      required: true,
    }
  },
  data() {
    return {
      followedUsers: [],
    }
  },
  methods: {
    doesFollowUsers(){
     return this.followedUsers.length > 0;
    },
    doesFollowUser(username) {
      return this.followedUsers.indexOf(username) > -1;
    },
    handleFollowUser(username) {
      const indexOfUser = this.followedUsers.indexOf(username);

      if (indexOfUser > -1){
        this.followedUsers.splice(indexOfUser, 1);
      } else {
        this.followedUsers.push(username)
      }
    },
    clearFollowedUsers() {
      this.followedUsers = [];
    },
    currentlyPaintingUser(username){
      return username === this.currentPaintingUser;
    }
  }
};
</script>

<template>
  <div id="users-sidebar">
    <h2>Users</h2>
    <UnfollowUsersButton :disabled="!doesFollowUsers" @click="clearFollowedUsers" class="unfollowButton"/>
    <div class="user-table">
      <div>&nbsp;</div>
      <div>Name</div>
      <div>Pixels/batch</div>
      <div>Avg response time</div>
      <div>&nbsp;</div>
      <UserDetails
        v-for="user in users"
        :currentPaintingUser="currentlyPaintingUser(user.username)"
        :user="user"
        :followsUser="doesFollowUser(user.username)"
        @handleFollowUser="handleFollowUser"/>
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

.unfollowButton {
  margin-bottom: 20px;
}

.user-table {
	display: grid;
	grid-template-rows: repeat(auto-fit, auto);
	grid-template-columns: 18px auto auto auto 18px;
	gap: 8px 8px;
}
</style>
