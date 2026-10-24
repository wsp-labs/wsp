// SPDX-License-Identifier: AGPL-3.0-only
declare const __BLOG_DRAFTS__: boolean;

declare module "*.md" {
  const post: import("./blog").Post;
  export default post;
}
