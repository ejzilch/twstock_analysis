FROM python:3.11-slim

WORKDIR /app

# Install dependencies first (cached layer)
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy source
COPY ai_service/ ./ai_service/

# Model file (mounted at runtime or baked in after training)
RUN mkdir -p ai_service/models

ENV MODEL_TYPE=rules
ENV PORT=8001

EXPOSE 8001

# Uvicorn with graceful shutdown support
CMD ["uvicorn", "ai_service.main:app", "--host", "0.0.0.0", "--port", "8001", "--timeout-graceful-shutdown", "30"]