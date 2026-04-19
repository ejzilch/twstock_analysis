/** @type {import('next').NextConfig} */
const nextConfig = {
    reactStrictMode: true,
    async rewrites() {
        console.log('API URL:' + process.env.NEXT_PUBLIC_API_BASE_URL);
        return [{ source: '/api/:path*', destination: `${process.env.NEXT_PUBLIC_API_BASE_URL}/api/:path*` }]
    },
}
module.exports = nextConfig