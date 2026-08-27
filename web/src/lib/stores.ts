import { writable } from "svelte/store";

export type Tag = { id: string; title: string; color: number; link?: string };
export type User = { id: string; email: string; role: string; link?: string };
export type App = { id: string; title: string; link?: string };

export type Video = {
    id: string;
    title: string;
    tags: Tag[];
    users: User[];
    apps: App[];
};

export let videos = writable<Video[]>([]);

