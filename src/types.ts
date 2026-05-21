export interface BookInfo {
  id: string;
  isbn: string;
  title: string;
  author: string;
  download_url: string;
  detail_url: string;
  publisher: string;
  language: string;
  year: string;
  extension: string;
  file_size: string;
  rating: string;
  quality: string;
  image_url: string;
}

export interface SearchResult {
  books: BookInfo[];
  total: number;
  page: number;
}

export interface DownloadProgress {
  progress: number;
  message: string;
  done: boolean;
}